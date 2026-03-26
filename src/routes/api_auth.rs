use std::sync::Arc;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;

use crate::auth::{self, sha256_hex};
use crate::config;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct LoginRequest {
    password: String,
}

fn is_https(headers: &HeaderMap) -> bool {
    headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .map_or(false, |v| v == "https")
}

async fn login(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<LoginRequest>,
) -> impl IntoResponse {
    let active_password = config::get_active_password(&state.settings, &state.db_pool);
    let input = payload.password.trim().to_string();

    match active_password {
        Some(ref pwd) if auth::verify_password_auto(&input, pwd.trim()) => {
            tracing::info!("登录成功");
            // Cookie value is sha256 of the plaintext input
            let hash = sha256_hex(&input);
            let cookie = auth::build_cookie(&hash, is_https(&headers));
            (
                [(axum::http::header::SET_COOKIE, cookie)],
                Json(serde_json::json!({
                    "status": "ok",
                    "message": "登录成功"
                })),
            )
                .into_response()
        }
        _ => {
            tracing::warn!("登录失败：密码错误");
            (
                axum::http::StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "status": "error",
                    "message": "密码错误"
                })),
            )
                .into_response()
        }
    }
}

async fn logout() -> impl IntoResponse {
    (
        [(axum::http::header::SET_COOKIE, auth::build_clear_cookie())],
        Json(serde_json::json!({
            "status": "ok",
            "message": "已退出登录"
        })),
    )
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/auth/login", post(login))
        .route("/api/auth/logout", post(logout))
}
