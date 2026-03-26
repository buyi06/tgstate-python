use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};

use crate::auth::{self, sha256_hex, COOKIE_NAME};
use crate::config;
use crate::error::error_payload;
use crate::state::AppState;

fn get_cookie_value(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    headers
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|c| {
                let c = c.trim();
                c.strip_prefix(&format!("{}=", name))
                    .map(|v| v.to_string())
            })
        })
}

/// CSRF check: for state-changing requests, verify Origin/Referer matches host.
fn check_csrf(request: &Request) -> bool {
    let method = request.method();
    if method == axum::http::Method::GET || method == axum::http::Method::HEAD || method == axum::http::Method::OPTIONS {
        return true;
    }

    let path = request.uri().path();

    // Skip CSRF for API-key based uploads (PicGo compatibility)
    if path.starts_with("/api/upload") {
        if request.headers().get("x-api-key").is_some() {
            return true;
        }
    }

    let host = request
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if let Some(origin) = request.headers().get("origin").and_then(|v| v.to_str().ok()) {
        if let Some(origin_host) = origin.strip_prefix("http://").or_else(|| origin.strip_prefix("https://")) {
            return origin_host == host;
        }
        return false;
    }

    if let Some(referer) = request.headers().get("referer").and_then(|v| v.to_str().ok()) {
        if let Some(after_scheme) = referer.strip_prefix("http://").or_else(|| referer.strip_prefix("https://")) {
            let referer_host = after_scheme.split('/').next().unwrap_or("");
            return referer_host == host;
        }
        return false;
    }

    true
}

/// Check if session cookie is valid against the active password.
/// Supports both hashed (argon2) and plaintext passwords.
fn check_session(session: Option<&str>, active_pwd: &str, app_settings: &config::AppSettingsMap) -> bool {
    let session = match session {
        Some(s) => s,
        None => return false,
    };

    // If session_token is stored (argon2 password), check against it
    if let Some(Some(token)) = app_settings.get("SESSION_TOKEN") {
        if !token.is_empty() {
            return auth::secure_compare(session, token);
        }
    }

    // Legacy: password is plaintext, cookie is sha256(password)
    let token = sha256_hex(active_pwd);
    auth::secure_compare(session, &token) || auth::secure_compare(session, active_pwd)
}

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();

    // CSRF check for all state-changing requests
    if !check_csrf(&request) {
        tracing::warn!("CSRF 检查失败: {}", path);
        return (
            StatusCode::FORBIDDEN,
            axum::Json(serde_json::json!({
                "status": "error",
                "code": "csrf_failed",
                "message": "跨域请求被拒绝"
            })),
        )
            .into_response();
    }

    // Static files and download paths: always public
    let static_public = ["/static", "/d", "/favicon.ico"];
    let is_static_public = static_public.iter().any(|p| path.starts_with(p));
    if is_static_public {
        return next.run(request).await;
    }

    let app_settings = config::get_app_settings(&state.settings, &state.db_pool);
    let active_password = app_settings
        .get("PASS_WORD")
        .and_then(|v| v.as_ref())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());

    // Also check env password as fallback
    let active_password = active_password.or_else(|| {
        config::get_active_password(&state.settings, &state.db_pool)
    });

    match active_password {
        None => {
            let public_no_auth = [
                "/welcome",
                "/settings",
                "/api/set-password",
                "/api/auth/",
                "/api/verify/",
                "/api/app-config",
            ];
            if public_no_auth.iter().any(|p| path.starts_with(p)) {
                return next.run(request).await;
            }
            return Redirect::temporary("/welcome").into_response();
        }
        Some(ref active_pwd) => {
            if path == "/welcome" {
                return Redirect::temporary("/").into_response();
            }

            let public_api = ["/api/auth/login", "/api/auth/logout", "/api/verify/"];
            if public_api.iter().any(|p| path.starts_with(p)) {
                return next.run(request).await;
            }

            if path.starts_with("/api/") {
                if path.starts_with("/api/upload") && request.headers().get("x-api-key").is_some() {
                    return next.run(request).await;
                }

                let session = get_cookie_value(request.headers(), COOKIE_NAME);
                let is_auth = check_session(session.as_deref(), active_pwd, &app_settings);

                if !is_auth {
                    tracing::warn!("API 未授权访问: {}", path);
                    return (
                        StatusCode::UNAUTHORIZED,
                        axum::Json(serde_json::json!({
                            "detail": error_payload("需要网页登录", "login_required", None)
                        })),
                    )
                        .into_response();
                }
                return next.run(request).await;
            }

            let protected_pages = ["/", "/image_hosting", "/files", "/settings"];
            let is_protected_page = protected_pages.iter().any(|p| path == *p);

            let session = get_cookie_value(request.headers(), COOKIE_NAME);
            let is_auth = check_session(session.as_deref(), active_pwd, &app_settings);

            if is_protected_page {
                if !is_auth {
                    return Redirect::temporary("/login").into_response();
                }
                return next.run(request).await;
            }

            if path == "/login" || path == "/pwd" {
                if is_auth {
                    return Redirect::temporary("/").into_response();
                }
                return next.run(request).await;
            }

            if path.starts_with("/share/") {
                return next.run(request).await;
            }

            next.run(request).await
        }
    }
}
