pub const COOKIE_NAME: &str = "tgstate_session";

use std::sync::OnceLock;

use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::constants;

pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

/// Generate a cryptographically random session token (32 bytes, hex-encoded -> 64 chars).
///
/// This is the canonical value stored in `app_settings.session_token` and set as the
/// session cookie. Because the token is independent of the password, cookies cannot be
/// predicted from the password, and rotating the password (or re-logging in) invalidates
/// prior sessions without touching the password hash.
pub fn generate_session_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn parse_truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Read and cache the `COOKIE_SECURE` env override. When set to a truthy value
/// (`1`/`true`/`yes`/`on`), session cookies are always marked `Secure` regardless
/// of request detection.
fn cookie_secure_override() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("COOKIE_SECURE")
            .map(|v| parse_truthy(&v))
            .unwrap_or(false)
    })
}

/// Read and cache the `SESSION_MAX_AGE_SECS` env override; fall back to the constant.
fn session_max_age_secs() -> u32 {
    static CACHED: OnceLock<u32> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("SESSION_MAX_AGE_SECS")
            .ok()
            .and_then(|v| v.trim().parse::<u32>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(constants::SESSION_MAX_AGE_SECS)
    })
}

#[cfg(test)]
mod tests {
    use super::{ensure_upload_auth, generate_session_token};

    #[test]
    fn password_only_api_request_without_session_is_rejected() {
        let result = ensure_upload_auth(false, None, None, Some("hashed"), None);
        match result {
            Err((401, _, "login_required")) => {}
            other => panic!("expected login_required rejection, got {:?}", other),
        }
    }

    #[test]
    fn password_only_request_with_matching_session_is_allowed() {
        let result = ensure_upload_auth(false, Some("hashed"), None, Some("hashed"), None);
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn password_set_referer_only_request_is_rejected() {
        // Referer alone must not grant upload access when a password is configured.
        let result = ensure_upload_auth(true, None, None, Some("hashed"), None);
        match result {
            Err((401, _, "login_required")) => {}
            other => panic!("expected login_required rejection, got {:?}", other),
        }
    }

    #[test]
    fn picgo_only_referer_only_request_is_rejected() {
        // Referer alone must not grant upload access when only a PicGo key is configured.
        let result = ensure_upload_auth(true, None, Some("secret"), None, None);
        match result {
            Err((401, _, "invalid_api_key")) => {}
            other => panic!("expected invalid_api_key rejection, got {:?}", other),
        }
    }

    #[test]
    fn generate_session_token_is_64_hex_chars() {
        let t = generate_session_token();
        assert_eq!(t.len(), 64);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
        // Two calls should differ with overwhelming probability.
        assert_ne!(t, generate_session_token());
    }
}

/// Build a session cookie string with security flags.
///
/// `is_https` is honored when true; the `COOKIE_SECURE` env var can force `Secure`
/// regardless. `SESSION_MAX_AGE_SECS` env controls the Max-Age (defaulting to
/// `constants::SESSION_MAX_AGE_SECS`).
pub fn build_cookie(value: &str, is_https: bool) -> String {
    let secure = if is_https || cookie_secure_override() {
        "; Secure"
    } else {
        ""
    };
    format!(
        "{}={}; HttpOnly; SameSite=Strict; Path=/; Max-Age={}{}",
        COOKIE_NAME,
        value,
        session_max_age_secs(),
        secure
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
///
/// `has_referer` is retained in the signature for call-site compatibility but no
/// longer grants any access on its own — a matching session cookie or submitted
/// key is always required when an API key or password is configured.
pub fn ensure_upload_auth(
    _has_referer: bool,
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

    // Only PICGO_API_KEY set: require matching submitted key.
    if has_picgo && !has_pwd {
        if let Some(key) = submitted_key {
            if secure_compare(key, picgo_api_key.unwrap()) {
                return Ok(());
            }
        }
        return Err((401, "无效的 API 密钥", "invalid_api_key"));
    }

    // Only PASS_WORD set: require matching session cookie.
    if !has_picgo && has_pwd {
        if let Some(cookie) = cookie_value {
            if secure_compare(cookie, pass_word.unwrap()) {
                return Ok(());
            }
        }
        return Err((401, "需要网页登录", "login_required"));
    }

    // Both set: accept either a valid session cookie OR a valid submitted key.
    if let Some(cookie) = cookie_value {
        if secure_compare(cookie, pass_word.unwrap()) {
            return Ok(());
        }
    }
    if let Some(key) = submitted_key {
        if secure_compare(key, picgo_api_key.unwrap()) {
            return Ok(());
        }
    }
    Err((401, "需要网页登录", "login_required"))
}
