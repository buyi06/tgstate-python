pub const COOKIE_NAME: &str = "tgstate_session";

use crate::constants;
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
        COOKIE_NAME, value, constants::SESSION_MAX_AGE_SECS, secure
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

/// Hash a password using argon2.
pub fn hash_password(password: &str) -> Result<String, String> {
    use argon2::password_hash::{rand_core::OsRng, PasswordHasher, SaltString};
    use argon2::Argon2;
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
}

/// Verify a password against an argon2 hash.
pub fn verify_password(password: &str, hash: &str) -> bool {
    use argon2::password_hash::PasswordVerifier;
    use argon2::{Argon2, PasswordHash};
    let parsed = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

/// Check if a stored value is an argon2 hash (vs plaintext).
pub fn is_hashed(stored: &str) -> bool {
    stored.starts_with("$argon2")
}

/// Verify password: auto-detect hashed vs plaintext.
pub fn verify_password_auto(input: &str, stored: &str) -> bool {
    if is_hashed(stored) {
        verify_password(input, stored)
    } else {
        secure_compare(input, stored)
    }
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
