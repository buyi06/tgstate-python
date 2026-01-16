import logging
import os
from fastapi import FastAPI, Request
from fastapi.staticfiles import StaticFiles
from starlette.templating import Jinja2Templates
from fastapi.responses import RedirectResponse, JSONResponse

# 导入我们的新生命周期管理器和路由
from .core.http_client import lifespan
from .api import routes as api_routes
from .pages import router as pages_router
from .core.config import get_active_password, get_app_settings
from .api.common import error_payload
from . import database

logging.basicConfig(
    level=os.getenv("LOG_LEVEL", "INFO").upper(),
    format="%(asctime)s %(levelname)s %(name)s: %(message)s",
)

# 使用集成的 lifespan 管理器创建 FastAPI 应用
app = FastAPI(
    lifespan=lifespan,
    title="tgState",
    description="一个基于 Telegram 的私有文件存储系统。",
    version="2.0.0"
)

COOKIE_NAME = "tgstate_session"

@app.middleware("http")
async def security_headers_middleware(request: Request, call_next):
    """
    Add security headers to all responses.
    """
    response = await call_next(request)
    response.headers["X-Content-Type-Options"] = "nosniff"
    response.headers["X-Frame-Options"] = "DENY"
    response.headers["Referrer-Policy"] = "no-referrer"
    
    # 简单的 Permissions-Policy
    response.headers["Permissions-Policy"] = "geolocation=(), microphone=(), camera=(), payment=(), usb=()"
    
    # Strict-Transport-Security (HSTS)
    # Only if HTTPS
    if request.url.scheme == "https" or request.headers.get("x-forwarded-proto") == "https":
        response.headers["Strict-Transport-Security"] = "max-age=31536000; includeSubDomains"
        
    return response

import hashlib

@app.middleware("http")
async def auth_middleware(request: Request, call_next):
    """
    一个全局中间件，用于处理所有页面的访问权限。
    """
    active_password = get_active_password()
    request_path = request.url.path

    # 定义公共路径，这些路径永远不拦截
    # /api/auth/login 用于登录，/api/auth/logout 用于登出，都应该放行
    public_paths = ["/static", "/api", "/d", "/favicon.ico"]
    is_public = any(request_path.startswith(p) for p in public_paths)

    # 情况 1：未设置密码
    if not active_password:
        # 如果是引导页或 API/静态资源，放行
        if request_path == "/welcome" or request_path == "/settings" or is_public:
            return await call_next(request)
        # 否则强制重定向到引导页
        return RedirectResponse(url="/welcome", status_code=307)

    # 情况 2：已设置密码
    # 如果访问引导页，强制跳转到主页
    if request_path == "/welcome":
        return RedirectResponse(url="/", status_code=307)

    # --- 鉴权逻辑 ---
    # 计算当前密码的 Hash
    active_token = hashlib.sha256(active_password.encode('utf-8')).hexdigest()
    
    session_password = request.cookies.get(COOKIE_NAME)
    # 兼容性检查：
    # 1. Cookie == SHA256(password) (新版逻辑)
    # 2. Cookie == password (旧版逻辑，避免升级踢下线)
    is_authenticated = (session_password == active_token) or (session_password == active_password)

    # 保护 API
    # 包含了上传、删除、文件列表、配置管理等敏感接口
    protected_api_prefixes = (
        "/api/upload", 
        "/api/delete", 
        "/api/files", 
        "/api/batch_delete", 
        "/api/app-config", 
        "/api/reset-config",
        "/api/set-password" 
    )
    
    if any(request_path.startswith(prefix) for prefix in protected_api_prefixes):
        if not is_authenticated:
            return JSONResponse(
                status_code=401,
                content={"detail": error_payload("需要网页登录", code="login_required")},
            )

    # 保护页面
    # 明确列出需要登录才能访问的页面
    protected_pages = ["/", "/image_hosting", "/files", "/settings"]
    
    # 登录页特殊处理：如果已登录，跳转到主页
    if request_path == "/login" or request_path == "/pwd":
        if is_authenticated:
            return RedirectResponse(url="/", status_code=307)
        # 未登录则允许访问登录页
        return await call_next(request)

    # 核心页面鉴权
    if request_path in protected_pages:
        if not is_authenticated:
            return RedirectResponse(url="/login", status_code=307)

    return await call_next(request)

# 挂载静态文件目录
# 注意：这个路径是相对于项目根目录的
app.mount("/static", StaticFiles(directory="app/static"), name="static")

# 设置模板目录
# 注意：这个路径也是相对于项目根目录的
templates = Jinja2Templates(directory="app/templates")

# 包含 API 和页面路由
app.include_router(api_routes.router)
app.include_router(pages_router)
