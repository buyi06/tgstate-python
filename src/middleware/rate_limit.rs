use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use tokio::sync::Mutex;

use crate::constants;

#[derive(Clone)]
struct RateEntry {
    count: u32,
    window_start: Instant,
}

#[derive(Clone)]
pub struct RateLimiter {
    /// (max_requests, window_duration)
    login: Arc<Mutex<HashMap<IpAddr, RateEntry>>>,
    upload: Arc<Mutex<HashMap<IpAddr, RateEntry>>>,
    api: Arc<Mutex<HashMap<IpAddr, RateEntry>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            login: Arc::new(Mutex::new(HashMap::new())),
            upload: Arc::new(Mutex::new(HashMap::new())),
            api: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

async fn check_rate(
    store: &Mutex<HashMap<IpAddr, RateEntry>>,
    ip: IpAddr,
    max_requests: u32,
    window: Duration,
) -> bool {
    let mut map = store.lock().await;
    let now = Instant::now();

    // Bound: if too many entries, evict expired ones first
    if map.len() > constants::RATE_LIMIT_MAX_ENTRIES {
        map.retain(|_, entry| now.duration_since(entry.window_start) < window);
        if map.len() > constants::RATE_LIMIT_MAX_ENTRIES {
            return false;
        }
    }

    let entry = map.entry(ip).or_insert(RateEntry {
        count: 0,
        window_start: now,
    });

    if now.duration_since(entry.window_start) > window {
        entry.count = 1;
        entry.window_start = now;
        true
    } else {
        entry.count += 1;
        entry.count <= max_requests
    }
}

fn extract_ip(request: &Request) -> IpAddr {
    // Try X-Forwarded-For first (reverse proxy)
    if let Some(xff) = request.headers().get("x-forwarded-for") {
        if let Ok(s) = xff.to_str() {
            if let Some(first) = s.split(',').next() {
                if let Ok(ip) = first.trim().parse::<IpAddr>() {
                    return ip;
                }
            }
        }
    }
    // Try X-Real-IP
    if let Some(xri) = request.headers().get("x-real-ip") {
        if let Ok(s) = xri.to_str() {
            if let Ok(ip) = s.trim().parse::<IpAddr>() {
                return ip;
            }
        }
    }
    // Fallback to loopback
    "127.0.0.1".parse().unwrap()
}

pub async fn rate_limit_middleware(
    State(limiter): State<RateLimiter>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();
    let ip = extract_ip(&request);

    let allowed = if path.starts_with("/api/auth/login") {
        check_rate(&limiter.login, ip, constants::RATE_LIMIT_LOGIN_MAX, Duration::from_secs(constants::RATE_LIMIT_WINDOW_SECS)).await
    } else if path.starts_with("/api/upload") {
        check_rate(&limiter.upload, ip, constants::RATE_LIMIT_UPLOAD_MAX, Duration::from_secs(constants::RATE_LIMIT_WINDOW_SECS)).await
    } else if path.starts_with("/api/") {
        check_rate(&limiter.api, ip, constants::RATE_LIMIT_API_MAX, Duration::from_secs(constants::RATE_LIMIT_WINDOW_SECS)).await
    } else {
        true
    };

    if !allowed {
        tracing::warn!("Rate limit exceeded for {} on {}", ip, path);
        return (
            StatusCode::TOO_MANY_REQUESTS,
            axum::Json(serde_json::json!({
                "status": "error",
                "code": "rate_limited",
                "message": "请求过于频繁，请稍后再试"
            })),
        )
            .into_response();
    }

    next.run(request).await
}

/// Periodically clean up expired entries (call from a background task)
pub async fn cleanup_expired(limiter: &RateLimiter) {
    let window = Duration::from_secs(constants::RATE_LIMIT_CLEANUP_INTERVAL_SECS);
    let now = Instant::now();

    for store in [&limiter.login, &limiter.upload, &limiter.api] {
        let mut map = store.lock().await;
        map.retain(|_, entry| now.duration_since(entry.window_start) < window);
    }
}
