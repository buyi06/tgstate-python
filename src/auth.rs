pub const COOKIE_NAME: &str = "tgstate_session";
pub const SESSION_MAX_AGE: u32 = 86400; // 24 hours

use sha2::{Digest, Sha256};

pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

/// Build a session cookie string with security flags.
pub fn build_cookie(value: &str, is_https: bool) -> String {
    let secure = if is_https { "; Secure" } else { "" };
    format!(
        "{}={}; HttpOnly; SameSite=Strict; Path=/; Max-Age={}{}",
        COOKIE_NAME, value, SESSION_MAX_AGE, secure
    )
}

/// Build a cookie that clears the session.
pub fn build_clear_cookie() -> String {
    format!(
        "{}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0",
        COOKIE_NAME
    )
}

/// Constant-time string comparison to prevent timing attacks.
pub fn secure_compare(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.as_bytes()
        .iter()
        .zip(b.as_bytes().iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

/// Check upload auth. Returns Ok(()) if allowed, Err(status_code, message, code) if not.
pub fn ensure_upload_auth(
    has_referer: bool,
    cookie_value: Option<&str>,
    picgo_api_key: Option<&str>,
    pass_word: Option<&str>,
    submitted_key: Option<&str>,
) -> Result<(), (u16, &'static str, &'static str)> {
    let has_picgo = picgo_api_key.map_or(false, |k| !k.is_empty());
    let has_pwd = pass_word.map_or(false, |p| !p.is_empty());

    // Neither set: allow all
    if !has_picgo && !has_pwd {
        return Ok(());
    }

    // Only PICGO_API_KEY set
    if has_picgo && !has_pwd {
        if has_referer {
            return Ok(());
        }
        if let Some(key) = submitted_key {
            if secure_compare(key, picgo_api_key.unwrap()) {
                return Ok(());
            }
        }
        return Err((401, "无效的 API 密钥", "invalid_api_key"));
    }

    // Only PASS_WORD set
    if !has_picgo && has_pwd {
        if !has_referer {
            return Ok(());
        }
        if let Some(cookie) = cookie_value {
            if secure_compare(cookie, pass_word.unwrap()) {
                return Ok(());
            }
        }
        return Err((401, "需要网页登录", "login_required"));
    }

    // Both set
    if has_referer {
        if let Some(cookie) = cookie_value {
            if secure_compare(cookie, pass_word.unwrap()) {
                return Ok(());
            }
        }
        return Err((401, "需要网页登录", "login_required"));
    }
    if let Some(key) = submitted_key {
        if secure_compare(key, picgo_api_key.unwrap()) {
            return Ok(());
        }
    }
    Err((401, "无效的 API 密钥", "invalid_api_key"))
}
