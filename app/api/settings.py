from __future__ import annotations

import logging
from fastapi import APIRouter, Request
from fastapi.responses import JSONResponse
from pydantic import BaseModel

from .common import http_error
from .. import database
from ..core.config import get_app_settings
from ..core.http_client import apply_runtime_settings

import telegram
from telegram.request import HTTPXRequest


router = APIRouter()
logger = logging.getLogger(__name__)


class PasswordRequest(BaseModel):
    password: str


class AppConfigRequest(BaseModel):
    BOT_TOKEN: str | None = None
    CHANNEL_NAME: str | None = None
    PASS_WORD: str | None = None
    BASE_URL: str | None = None
    PICGO_API_KEY: str | None = None


def _validate_config(cfg: dict) -> None:
    token = (cfg.get("BOT_TOKEN") or "").strip()
    if token and (":" not in token or len(token) < 20):
        raise http_error(400, "BOT_TOKEN 格式不正确", code="invalid_bot_token")

    channel = (cfg.get("CHANNEL_NAME") or "").strip()
    if channel and not (channel.startswith("@") or channel.startswith("-100")):
        raise http_error(400, "CHANNEL_NAME 格式不正确（@username 或 -100...）", code="invalid_channel")

    base_url = (cfg.get("BASE_URL") or "").strip()
    if base_url and not (base_url.startswith("http://") or base_url.startswith("https://")):
        raise http_error(400, "BASE_URL 必须以 http:// 或 https:// 开头", code="invalid_base_url")


@router.get("/api/app-config")
async def get_app_config(request: Request):
    cfg = get_app_settings()
    bot_ready = bool(getattr(request.app.state, "bot_ready", False))
    return {
        "status": "ok",
        "cfg": {
            "BOT_TOKEN_SET": bool((cfg.get("BOT_TOKEN") or "").strip()),
            "CHANNEL_NAME": cfg.get("CHANNEL_NAME") or "",
            "PASS_WORD_SET": bool((cfg.get("PASS_WORD") or "").strip()),
            "BASE_URL": cfg.get("BASE_URL") or "",
            "PICGO_API_KEY_SET": bool((cfg.get("PICGO_API_KEY") or "").strip()),
        },
        "bot": {
            "ready": bot_ready,
            "running": bool(getattr(request.app.state, "bot_app", None)),
            "error": getattr(request.app.state, "bot_error", None),
        },
    }


def _merge_config(existing: dict, incoming: dict) -> dict:
    merged = dict(existing)
    for k, v in incoming.items():
        if v is None:
            continue
        if isinstance(v, str):
            # 允许保存空字符串（用于清空配置）
            merged[k] = v.strip()
        else:
            merged[k] = v
    return merged


@router.post("/api/app-config/save")
async def save_config_only(payload: AppConfigRequest, request: Request):
    existing = database.get_app_settings_from_db()
    incoming = payload.model_dump()
    merged = _merge_config(existing, incoming)

    # Partial validation is implicit in _validate_config (it skips empty values)
    _validate_config(merged)
    database.save_app_settings_to_db(merged)
    logger.info("配置已保存（未应用）")
    return {"status": "ok", "message": "已保存（未应用）"}


@router.post("/api/app-config/apply")
async def save_and_apply(payload: AppConfigRequest, request: Request):
    existing = database.get_app_settings_from_db()
    incoming = payload.model_dump()
    merged = _merge_config(existing, incoming)
    _validate_config(merged)
    database.save_app_settings_to_db(merged)

    # 只有当 BOT_TOKEN 和 CHANNEL_NAME 都存在时才尝试启动 Bot
    # 但 Web 设置无论如何都会保存生效
    await apply_runtime_settings(request.app, start_bot=True)
    logger.info("配置已保存并应用")

    resp = JSONResponse(
        status_code=200,
        content={
            "status": "ok",
            "message": "已保存并应用",
            "bot": {
                "ready": bool(getattr(request.app.state, "bot_ready", False)),
                "running": bool(getattr(request.app.state, "bot_app", None)),
            },
        },
    )
    
import hashlib

    pwd = (merged.get("PASS_WORD") or "").strip()
    if pwd:
        # 修改密码或保存配置时，如果涉及密码变更，更新 Cookie
        # 统一使用 tgstate_session 名称
        # 修复：不再存储明文密码，改存 SHA256 哈希
        token = hashlib.sha256(pwd.encode('utf-8')).hexdigest()
        resp.set_cookie(key="tgstate_session", value=token, httponly=True, samesite="Lax", path="/")
    else:
        resp.delete_cookie("tgstate_session", path="/", httponly=True, samesite="Lax")
        
    return resp


@router.post("/api/reset-config")
async def reset_config(request: Request):
    database.reset_app_settings_in_db()
    await apply_runtime_settings(request.app, start_bot=True)
    logger.warning("配置已重置")
    resp = JSONResponse(status_code=200, content={"status": "ok", "message": "配置已重置"})
    resp.delete_cookie("tgstate_session", path="/", httponly=True, samesite="Lax")
    return resp


@router.post("/api/set-password")
async def set_password(payload: PasswordRequest, request: Request):
    try:
        current = get_app_settings()
        pwd = (payload.password or "").strip()
        database.save_app_settings_to_db({**current, "PASS_WORD": pwd})
        await apply_runtime_settings(request.app, start_bot=False)
        logger.info("密码已更新")
        return {"status": "ok", "message": "密码已成功设置。"}
    except Exception as e:
        logger.error("写入密码失败: %s", e)
        raise http_error(500, "无法写入密码。", code="write_password_failed", details=str(e))


class VerifyRequest(BaseModel):
    BOT_TOKEN: str | None = None
    CHANNEL_NAME: str | None = None


@router.post("/api/verify/bot")
async def verify_bot(payload: VerifyRequest):
    token = (payload.BOT_TOKEN or "").strip()
    if not token:
        settings = get_app_settings()
        token = (settings.get("BOT_TOKEN") or "").strip()
    if not token:
        return {"status": "ok", "available": False, "message": "未提供 BOT_TOKEN"}

    # _validate_config({"BOT_TOKEN": token})  # 暂时跳过严格格式验证，让 Telegram API 决定
    req = HTTPXRequest(connect_timeout=10.0, read_timeout=10.0, write_timeout=10.0)
    bot = telegram.Bot(token=token, request=req)
    try:
        me = await bot.get_me()
        return {"status": "ok", "ok": True, "available": True, "result": {"username": getattr(me, "username", None)}}
    except Exception as e:
        return {"status": "ok", "ok": False, "available": False, "message": str(e)}


@router.post("/api/verify/channel")
async def verify_channel(payload: VerifyRequest):
    token = (payload.BOT_TOKEN or "").strip()
    channel = (payload.CHANNEL_NAME or "").strip()

    if not token or not channel:
        settings = get_app_settings()
        token = token or (settings.get("BOT_TOKEN") or "").strip()
        channel = channel or (settings.get("CHANNEL_NAME") or "").strip()

    if not token or not channel:
        return {"status": "ok", "available": False, "message": "未提供 BOT_TOKEN 或 CHANNEL_NAME"}

    _validate_config({"BOT_TOKEN": token, "CHANNEL_NAME": channel})
    req = HTTPXRequest(connect_timeout=10.0, read_timeout=10.0, write_timeout=10.0)
    bot = telegram.Bot(token=token, request=req)
    try:
        msg = await bot.send_message(chat_id=channel, text="tgState channel check")
        try:
            await bot.delete_message(chat_id=channel, message_id=msg.message_id)
        except Exception:
            pass
        return {"status": "ok", "available": True}
    except Exception as e:
        return {"status": "ok", "available": False, "message": str(e)}


