use std::sync::Arc;

use axum::extract::{Multipart, State};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use bytes::BytesMut;

use crate::auth::{self, COOKIE_NAME};
use crate::config;
use crate::constants;
use crate::database;
use crate::error::http_error;
use crate::state::AppState;
use crate::telegram::service::TelegramService;

#[derive(Debug, Default)]
struct UploadAuthProgress {
    auth_verified: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum UploadFieldError {
    FileBeforeAuth,
}

fn advance_upload_auth_state(
    mut state: UploadAuthProgress,
    prechecked_auth: bool,
    auth_optional: bool,
    field_name: &str,
    _field_value: Option<&str>,
) -> Result<UploadAuthProgress, UploadFieldError> {
    if prechecked_auth || auth_optional {
        state.auth_verified = true;
        return Ok(state);
    }

    if field_name == "key" {
        state.auth_verified = true;
        return Ok(state);
    }

    if field_name == "file" && !state.auth_verified {
        return Err(UploadFieldError::FileBeforeAuth);
    }

    Ok(state)
}

/// Sanitize filename: extract basename, limit length, remove dangerous chars.
fn sanitize_filename(raw: &str) -> String {
    let name = std::path::Path::new(raw)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("upload");
    let clean: String = name
        .chars()
        .filter(|c| !c.is_control() && *c != '\0')
        .collect();
    if clean.len() > 255 {
        clean[..255].to_string()
    } else if clean.is_empty() {
        "upload".to_string()
    } else {
        clean
    }
}

async fn upload_file(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let app_settings = config::get_app_settings(&state.settings, &state.db_pool);

    let bot_token = app_settings
        .get("BOT_TOKEN")
        .and_then(|v| v.as_deref())
        .unwrap_or("");
    let channel_name = app_settings
        .get("CHANNEL_NAME")
        .and_then(|v| v.as_deref())
        .unwrap_or("");

    if bot_token.is_empty() || channel_name.is_empty() {
        return Err(http_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "缺少 BOT_TOKEN 或 CHANNEL_NAME，无法上传",
            "cfg_missing",
        ));
    }

    // Pre-check auth with header-only info (before consuming body)
    let has_referer = headers.get("referer").is_some();
    let cookie_value = headers
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|c| {
                let c = c.trim();
                c.strip_prefix(&format!("{}=", COOKIE_NAME))
                    .map(|v| v.to_string())
            })
        });

    let picgo_key = app_settings.get("PICGO_API_KEY").and_then(|v| v.as_deref());
    let pass_word = app_settings.get("PASS_WORD").and_then(|v| v.as_deref());
    let pass_word_hash = pass_word.map(|p| auth::sha256_hex(p));
    let pass_word_hash_ref = pass_word_hash.as_deref();

    let header_key = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let auth_optional = picgo_key.map_or(true, |k| k.is_empty())
        && pass_word_hash_ref.map_or(true, |p| p.is_empty());
    // Pre-check auth using only HEADER-available credentials: the session
    // cookie (browser login) and/or x-api-key (PicGo / API clients). These
    // are the only credentials that exist before we consume the multipart
    // body. Referer is client-controlled and auth.rs ignores it.
    //
    // The browser session cookie is the random SESSION_TOKEN from
    // app_settings, NOT the password. We verify it directly against the
    // stored token so a logged-in browser can upload without submitting a
    // form `key` field. `auth_middleware` uses the same comparison.
    let session_token_owned = app_settings
        .get("SESSION_TOKEN")
        .and_then(|v| v.clone());
    let cookie_valid = match (cookie_value.as_deref(), session_token_owned.as_deref()) {
        (Some(c), Some(t)) if !c.is_empty() && !t.is_empty() => {
            auth::secure_compare(c, t)
        }
        _ => false,
    };
    // ensure_upload_auth handles the x-api-key / PicGo path. We pass None
    // for the cookie because cookie validity is handled via `cookie_valid`
    // above — that function still compares cookie to the password hash,
    // which does not match the random session token used in v2.x.
    let prechecked_auth = cookie_valid
        || auth::ensure_upload_auth(
            has_referer,
            None,
            picgo_key,
            pass_word_hash_ref,
            header_key.as_deref(),
        )
        .is_ok();

    // Parse multipart body - stream file chunks to Telegram
    let mut form_key: Option<String> = None;
    let mut upload_result: Option<Result<String, String>> = None;
    let mut auth_progress = UploadAuthProgress {
        auth_verified: prechecked_auth || auth_optional,
    };

    let tg_service = TelegramService::new(
        bot_token.to_string(),
        channel_name.to_string(),
        state.http_client.clone(),
    );

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name == "key" {
            let key_text = field.text().await.ok();
            if !auth_progress.auth_verified {
                if let Err((_, msg, code)) = auth::ensure_upload_auth(
                    has_referer,
                    None,
                    picgo_key,
                    pass_word_hash_ref,
                    key_text.as_deref(),
                ) {
                    return Err(http_error(axum::http::StatusCode::UNAUTHORIZED, msg, code));
                }
            }
            auth_progress = advance_upload_auth_state(
                auth_progress,
                prechecked_auth,
                auth_optional,
                &name,
                key_text.as_deref(),
            )
            .map_err(|_| {
                http_error(
                    axum::http::StatusCode::UNAUTHORIZED,
                    "upload auth required before file field",
                    "file_before_auth",
                )
            })?;
            form_key = key_text;
        } else if name == "file" {
            auth_progress = advance_upload_auth_state(
                auth_progress,
                prechecked_auth,
                auth_optional,
                &name,
                None,
            )
            .map_err(|_| {
                http_error(
                    axum::http::StatusCode::UNAUTHORIZED,
                    "upload auth required before file field",
                    "file_before_auth",
                )
            })?;
            let raw_filename = field.file_name().unwrap_or("upload").to_string();
            let filename = sanitize_filename(&raw_filename);

            // Stream the file in chunks to Telegram
            upload_result = Some(
                stream_upload_to_telegram(&tg_service, field, &filename, &state.db_pool).await,
            );
        }
    }

    // Final auth check with form-level `key`. Only needed when header-level
    // credentials (cookie / x-api-key) did not already satisfy auth — e.g.
    // PicGo clients that authenticate by submitting PICGO_API_KEY in the
    // multipart body instead of as a header.
    if !prechecked_auth {
        let final_key = form_key.as_deref();
        if let Err((_, msg, code)) = auth::ensure_upload_auth(
            has_referer,
            None,
            picgo_key,
            pass_word_hash_ref,
            final_key,
        ) {
            return Err(http_error(axum::http::StatusCode::UNAUTHORIZED, msg, code));
        }
    }

    let short_id = upload_result
        .ok_or_else(|| http_error(axum::http::StatusCode::BAD_REQUEST, "未提供文件", "no_file"))?
        .map_err(|e| {
            tracing::error!("文件上传失败: {}", e);
            http_error(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "文件上传失败",
                "upload_failed",
            )
        })?;

    let download_path = format!("/d/{}", short_id);
    Ok(Json(serde_json::json!({
        "file_id": short_id,
        "short_id": short_id,
        "download_path": download_path,
        "path": download_path,
        "url": download_path,
    })))
}

/// Stream file upload: reads multipart field in chunks, uploads each chunk to Telegram
/// as it reaches TELEGRAM_CHUNK_SIZE. Peak memory is ~1 chunk (~20MB) instead of the full file.
async fn stream_upload_to_telegram(
    tg_service: &TelegramService,
    mut field: axum::extract::multipart::Field<'_>,
    filename: &str,
    db_pool: &database::DbPool,
) -> Result<String, String> {
    let chunk_size = constants::TELEGRAM_CHUNK_SIZE;
    let mut buffer = BytesMut::with_capacity(chunk_size);
    let mut total_size: usize = 0;
    let mut chunk_ids: Vec<String> = Vec::new();
    let mut first_message_id: Option<i64> = None;
    let mut chunk_num: u32 = 0;

    // Read field data incrementally
    while let Ok(Some(bytes)) = field.chunk().await {
        buffer.extend_from_slice(&bytes);
        total_size += bytes.len();

        // When buffer reaches chunk size, send it
        while buffer.len() >= chunk_size {
            chunk_num += 1;
            let chunk_data = buffer.split_to(chunk_size).freeze().to_vec();
            let chunk_name = format!("{}.part{}", filename, chunk_num);

            let message = tg_service
                .send_document_raw(chunk_data, &chunk_name, first_message_id)
                .await?;

            if first_message_id.is_none() {
                first_message_id = Some(message.message_id);
            }

            let doc = message.document.ok_or("No document in chunk response")?;
            chunk_ids.push(format!("{}:{}", message.message_id, doc.file_id));
        }
    }

    // Handle remaining data in buffer
    if buffer.is_empty() && chunk_ids.is_empty() {
        return Err("文件为空".into());
    }

    if chunk_ids.is_empty() {
        // Small file: single upload (no chunks were sent yet)
        tracing::info!("直接上传文件: {} ({}字节)", filename, total_size);
        let data = buffer.freeze().to_vec();
        let message = tg_service.send_document_raw(data, filename, None).await?;

        let doc = message.document.ok_or("No document in response")?;
        let composite_id = format!("{}:{}", message.message_id, doc.file_id);

        let short_id =
            database::add_file_metadata(db_pool, filename, &composite_id, total_size as i64)
                .map_err(|e| e.to_string())?;
        return Ok(short_id);
    }

    // Send remaining buffer as last chunk
    if !buffer.is_empty() {
        chunk_num += 1;
        let chunk_data = buffer.freeze().to_vec();
        let chunk_name = format!("{}.part{}", filename, chunk_num);

        let message = tg_service
            .send_document_raw(chunk_data, &chunk_name, first_message_id)
            .await?;

        let doc = message
            .document
            .ok_or("No document in last chunk response")?;
        chunk_ids.push(format!("{}:{}", message.message_id, doc.file_id));
    }

    // Multi-chunk: create and upload manifest
    tracing::info!(
        "分块上传完成: {} ({}MB, {} 块)",
        filename,
        total_size / (1024 * 1024),
        chunk_ids.len()
    );

    let mut manifest = String::from("tgstate-blob\n");
    manifest.push_str(filename);
    manifest.push('\n');
    for cid in &chunk_ids {
        manifest.push_str(cid);
        manifest.push('\n');
    }

    let manifest_name = format!("{}.manifest", filename);
    let message = tg_service
        .send_document_raw(manifest.into_bytes(), &manifest_name, first_message_id)
        .await?;

    let doc = message.document.ok_or("No document in manifest response")?;
    let manifest_composite = format!("{}:{}", message.message_id, doc.file_id);

    let short_id =
        database::add_file_metadata(db_pool, filename, &manifest_composite, total_size as i64)
            .map_err(|e| e.to_string())?;
    Ok(short_id)
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/upload", post(upload_file))
}

#[cfg(test)]
mod tests {
    use super::{advance_upload_auth_state, UploadAuthProgress, UploadFieldError};
    use crate::config::Settings;
    use crate::database;
    use crate::state::AppState;
    use axum::body::{to_bytes, Body};
    use axum::http::{header, Request, StatusCode};
    use axum::Router;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tower::util::ServiceExt;

    fn test_state() -> Arc<AppState> {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let data_dir = std::env::temp_dir()
            .join(format!("tgstate-upload-test-{}", unique))
            .to_string_lossy()
            .to_string();

        let settings = Settings {
            bot_token: Some("123456:test-token".into()),
            channel_name: Some("@test_channel".into()),
            pass_word: Some("secret".into()),
            picgo_api_key: None,
            base_url: "http://127.0.0.1:8000".into(),
            _mode: "p".into(),
            _file_route: "/d/".into(),
            data_dir: data_dir.clone(),
        };

        let db_pool = database::init_db(&data_dir);
        let tera = tera::Tera::default();
        let http_client = reqwest::Client::new();
        let app_settings = crate::config::get_app_settings(&settings, &db_pool);
        Arc::new(AppState::new(
            settings,
            tera,
            http_client,
            db_pool,
            app_settings,
            true,
        ))
    }

    fn multipart_request_with_file_before_key() -> Request<Body> {
        let boundary = "X-BOUNDARY";
        let body = format!(
            "--{b}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"test.txt\"\r\nContent-Type: text/plain\r\n\r\nhello\r\n--{b}\r\nContent-Disposition: form-data; name=\"key\"\r\n\r\nsecret\r\n--{b}--\r\n",
            b = boundary
        );

        Request::builder()
            .method("POST")
            .uri("/api/upload")
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(Body::from(body))
            .unwrap()
    }

    #[test]
    fn upload_requires_key_before_file_for_api_requests() {
        let state = UploadAuthProgress::default();
        let result = advance_upload_auth_state(state, false, false, "file", None);
        assert!(matches!(result, Err(UploadFieldError::FileBeforeAuth)));
    }

    #[test]
    fn upload_accepts_key_before_file_for_api_requests() {
        let state = UploadAuthProgress::default();
        let state = advance_upload_auth_state(state, false, false, "key", Some("secret")).unwrap();
        let state = advance_upload_auth_state(state, false, false, "file", None).unwrap();
        assert!(state.auth_verified);
    }

    #[tokio::test]
    async fn upload_route_rejects_file_field_before_auth() {
        let state = test_state();
        let app = Router::new()
            .merge(super::router())
            .with_state(state.clone());
        let response = app
            .oneshot(multipart_request_with_file_before_key())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains("file_before_auth"), "unexpected body: {}", text);

        let files = database::get_all_files(&state.db_pool).unwrap();
        assert!(files.is_empty(), "unexpected files persisted: {:?}", files);
    }
}
