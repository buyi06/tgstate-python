/// Maximum upload body size (512 MB)
pub const MAX_UPLOAD_BODY_SIZE: usize = 512 * 1024 * 1024;

/// Telegram chunk size for large file uploads (~19.5 MB, under Telegram's 20MB limit)
pub const TELEGRAM_CHUNK_SIZE: usize = (19.5 * 1024.0 * 1024.0) as usize;

/// HTTP client timeout for file upload/download operations (seconds)
pub const HTTP_TIMEOUT_TRANSFER_SECS: u64 = 300;

/// HTTP client timeout for metadata/API operations (seconds)
pub const HTTP_TIMEOUT_METADATA_SECS: u64 = 30;

/// Rate limit: login attempts per window
pub const RATE_LIMIT_LOGIN_MAX: u32 = 5;
/// Rate limit: upload requests per window
pub const RATE_LIMIT_UPLOAD_MAX: u32 = 10;
/// Rate limit: general API requests per window
pub const RATE_LIMIT_API_MAX: u32 = 120;
/// Rate limit: window duration in seconds
pub const RATE_LIMIT_WINDOW_SECS: u64 = 60;

/// Rate limiter cleanup interval in seconds
pub const RATE_LIMIT_CLEANUP_INTERVAL_SECS: u64 = 120;

/// Maximum entries per rate limiter bucket before forced eviction
pub const RATE_LIMIT_MAX_ENTRIES: usize = 10_000;

/// SSE keepalive interval in seconds
pub const SSE_KEEPALIVE_SECS: u64 = 15;

/// Session cookie max-age in seconds (24 hours)
pub const SESSION_MAX_AGE_SECS: u32 = 86400;

/// Short ID length for file identifiers
pub const SHORT_ID_LENGTH: usize = 6;

/// Broadcast event bus capacity
pub const EVENT_BUS_CAPACITY: usize = 200;

/// Bot polling long-poll timeout in seconds
pub const BOT_POLL_TIMEOUT_SECS: u64 = 30;
