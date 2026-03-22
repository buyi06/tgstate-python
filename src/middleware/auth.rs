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
    // Only check POST, PUT, DELETE, PATCH
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

    // Get Host header
    let host = request
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    // Check Origin header first
    if let Some(origin) = request.headers().get("origin").and_then(|v| v.to_str().ok()) {
        // Parse origin to extract host part
        if let Some(origin_host) = origin.strip_prefix("http://").or_else(|| origin.strip_prefix("https://")) {
            return origin_host == host;
        }
        return false;
    }

    // Fallback: check Referer header
    if let Some(referer) = request.headers().get("referer").and_then(|v| v.to_str().ok()) {
        if let Some(after_scheme) = referer.strip_prefix("http://").or_else(|| referer.strip_prefix("https://")) {
            let referer_host = after_scheme.split('/').next().unwrap_or("");
            return referer_host == host;
        }
        return false;
    }

    // No Origin or Referer: allow (for non-browser clients like curl)
    true
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

    let active_password = config::get_active_password(&state.settings);

    match active_password {
        None => {
            // No password set: only allow /welcome, /settings, and public API paths
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
            // Password is set
            if path == "/welcome" {
                return Redirect::temporary("/").into_response();
            }

            // Public API endpoints (no auth needed)
            let public_api = ["/api/auth/login", "/api/auth/logout", "/api/verify/"];
            if public_api.iter().any(|p| path.starts_with(p)) {
                return next.run(request).await;
            }

            // All other /api/* endpoints require auth
            if path.starts_with("/api/") {
                let session = get_cookie_value(request.headers(), COOKIE_NAME);
                let token = sha256_hex(active_pwd);
                let is_auth = session
                    .as_ref()
                    .map_or(false, |s| auth::secure_compare(s, &token) || auth::secure_compare(s, active_pwd));

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

            // Protected pages
            let protected_pages = ["/", "/image_hosting", "/files", "/settings"];
            let is_protected_page = protected_pages.iter().any(|p| path == *p);

            let session = get_cookie_value(request.headers(), COOKIE_NAME);
            let token = sha256_hex(active_pwd);
            let is_auth = session
                .as_ref()
                .map_or(false, |s| auth::secure_compare(s, &token) || auth::secure_compare(s, active_pwd));

            if is_protected_page {
                if !is_auth {
                    return Redirect::temporary("/login").into_response();
                }
                return next.run(request).await;
            }

            // Login pages: redirect to / if already authenticated
            if path == "/login" || path == "/pwd" {
                if is_auth {
                    return Redirect::temporary("/").into_response();
                }
                return next.run(request).await;
            }

            // Share pages: public
            if path.starts_with("/share/") {
                return next.run(request).await;
            }

            // Everything else: pass through
            next.run(request).await
        }
    }
}
