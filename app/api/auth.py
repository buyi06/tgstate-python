from fastapi import APIRouter, Response, Request
from fastapi.responses import JSONResponse
from pydantic import BaseModel
from ..core.config import get_active_password
import hashlib

router = APIRouter()

class LoginRequest(BaseModel):
    password: str

COOKIE_NAME = "tgstate_session"

@router.post("/api/auth/login")
async def login(payload: LoginRequest, response: Response):
    active_password = get_active_password()
    # 确保密码比对时处理两端空格，避免复制粘贴带来的隐形字符问题
    input_pwd = payload.password.strip()
    stored_pwd = (active_password or "").strip()
    
    if input_pwd and input_pwd == stored_pwd:
        # 登录成功，设置 Cookie
        # 修复：不再存储明文密码，改存 SHA256 哈希，避免特殊字符导致 500 错误
        token = hashlib.sha256(stored_pwd.encode('utf-8')).hexdigest()
        
        response = JSONResponse(content={"status": "ok", "message": "登录成功"})
        # 关键修复：设置 secure=False 以支持 http://IP:PORT 访问
        # samesite="Lax" 允许在同一站点导航时发送 Cookie
        response.set_cookie(
            key=COOKIE_NAME,
            value=token,
            httponly=True,
            samesite="Lax",
            path="/",
            secure=False # 兼容非 HTTPS 环境
        )
        return response
    else:
        return JSONResponse(status_code=401, content={"status": "error", "message": "密码错误"})

@router.post("/api/auth/logout")
async def logout():
    # 登出，清除 Cookie
    # 修复：不依赖 response 参数，而是直接返回一个新的 Response
    response = JSONResponse(content={"status": "ok", "message": "已退出登录"})
    response.delete_cookie(key=COOKIE_NAME, path="/", httponly=True, samesite="Lax")
    return response
