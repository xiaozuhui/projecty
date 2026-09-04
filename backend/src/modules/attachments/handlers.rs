use axum::{
    Json,
    body::{Body, Bytes},
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use crate::{
    http::{
        error::{ApiEnvelope, AppError, success},
        extractors::CurrentUser,
    },
    modules::attachments::service::{self, AttachmentError, AttachmentView, UploadSessionView},
    state::AppState,
};

fn map_error(error: AttachmentError) -> AppError {
    match error {
        AttachmentError::NotFound => AppError {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: "附件不存在".to_owned(),
        },
        AttachmentError::SessionNotFound => AppError {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: "上传会话不存在或已过期".to_owned(),
        },
        AttachmentError::Forbidden => AppError {
            status: StatusCode::FORBIDDEN,
            code: "forbidden",
            message: "没有当前任务的附件权限".to_owned(),
        },
        AttachmentError::InvalidInput(message) => AppError::bad_request(message),
        AttachmentError::Database(error) => {
            tracing::error!(?error, "attachment operation failed");
            AppError::internal("附件服务暂时不可用")
        }
        AttachmentError::Io(error) => {
            tracing::error!(%error, "attachment storage failed");
            AppError::internal("附件存储暂时不可用")
        }
    }
}

/// 单发上传(legacy):multipart 字段流式写入暂存文件,不在内存里缓冲整个文件。
pub async fn upload(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(task_key): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<ApiEnvelope<AttachmentView>>, AppError> {
    let max_bytes = crate::config::UPLOAD_MAX_BYTES;
    let staged_path = service::legacy_staged_path(&state.config.upload_dir);
    if let Some(parent) = staged_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| AppError::internal(format!("附件存储暂时不可用：{error}")))?;
    }
    let mut staged: Option<(Option<String>, Option<String>, u64, Vec<u8>, String)> = None;
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|error| AppError::bad_request(format!("文件上传失败：{error}")))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let name = field.file_name().map(|value| value.to_owned());
        let content_type = field.content_type().map(|value| value.to_owned());
        let mut writer = tokio::fs::File::create(&staged_path)
            .await
            .map_err(|error| AppError::internal(format!("附件存储暂时不可用：{error}")))?;
        let mut hasher = Sha256::new();
        let mut first_bytes = Vec::new();
        let mut byte_size: u64 = 0;
        let mut oversize = false;
        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|error| AppError::bad_request(format!("文件读取失败：{error}")))?
        {
            byte_size += chunk.len() as u64;
            if byte_size > max_bytes as u64 {
                oversize = true;
                break;
            }
            hasher.update(&chunk);
            if first_bytes.len() < 32 {
                let take = (32 - first_bytes.len()).min(chunk.len());
                first_bytes.extend_from_slice(&chunk[..take]);
            }
            writer
                .write_all(&chunk)
                .await
                .map_err(|error| AppError::internal(format!("附件存储暂时不可用：{error}")))?;
        }
        writer
            .flush()
            .await
            .map_err(|error| AppError::internal(format!("附件存储暂时不可用：{error}")))?;
        drop(writer);
        if oversize {
            let _ = tokio::fs::remove_file(&staged_path).await;
            return Err(AppError::bad_request(format!(
                "文件大小不能超过 {} MB",
                max_bytes / 1024 / 1024
            )));
        }
        staged = Some((
            name,
            content_type,
            byte_size,
            first_bytes,
            service::hex_digest(&hasher.finalize()),
        ));
        break;
    }
    let (name, content_type, byte_size, first_bytes, sha256_hex) =
        staged.ok_or_else(|| AppError::bad_request("需要上传名为 file 的文件"))?;
    let view = service::upload(
        &state.db,
        &current_user,
        &task_key,
        service::IncomingStaged {
            staged_path,
            byte_size,
            first_bytes,
            sha256_hex,
            name,
            content_type,
        },
        &state.config.upload_dir,
        max_bytes,
    )
    .await
    .map_err(map_error)?;
    Ok(success(view))
}

pub async fn list(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(task_key): Path<String>,
) -> Result<Json<ApiEnvelope<Vec<AttachmentView>>>, AppError> {
    let views = service::list(&state.db, &current_user, &task_key)
        .await
        .map_err(map_error)?;
    Ok(success(views))
}

#[derive(serde::Deserialize)]
pub struct InitUploadRequest {
    pub file_name: String,
    pub mime_type: Option<String>,
    pub total_bytes: i64,
    pub client_file_key: String,
    pub sha256: Option<String>,
}

/// 建立分片上传会话:同文件指纹的未完成会话直接续用(断点续传)。
pub async fn init_upload(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(task_key): Path<String>,
    Json(request): Json<InitUploadRequest>,
) -> Result<Json<ApiEnvelope<UploadSessionView>>, AppError> {
    let view = service::init_upload(
        &state.db,
        &current_user,
        &task_key,
        service::InitUploadInput {
            file_name: request.file_name,
            mime_type: request.mime_type,
            total_bytes: request.total_bytes,
            client_file_key: request.client_file_key,
            client_sha256: request.sha256,
        },
        &state.config.upload_dir,
        crate::config::UPLOAD_MAX_BYTES,
        crate::config::UPLOAD_CHUNK_BYTES,
    )
    .await
    .map_err(map_error)?;
    Ok(success(view))
}

/// 上传单个分片:原始二进制请求体(非 multipart),可选 X-Checksum-Sha256 头校验。
pub async fn upload_chunk(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path((upload_id, index)): Path<(Uuid, i32)>,
    headers: HeaderMap,
    bytes: Bytes,
) -> Result<Json<ApiEnvelope<UploadSessionView>>, AppError> {
    let checksum = headers
        .get("X-Checksum-Sha256")
        .and_then(|value| value.to_str().ok());
    let view = service::upload_chunk(
        &state.db,
        &current_user,
        upload_id,
        index,
        &bytes,
        checksum,
        &state.config.upload_dir,
    )
    .await
    .map_err(map_error)?;
    Ok(success(view))
}

pub async fn upload_state(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(upload_id): Path<Uuid>,
) -> Result<Json<ApiEnvelope<UploadSessionView>>, AppError> {
    let view = service::upload_session_state(&state.db, &current_user, upload_id)
        .await
        .map_err(map_error)?;
    Ok(success(view))
}

pub async fn complete_upload(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(upload_id): Path<Uuid>,
) -> Result<Json<ApiEnvelope<AttachmentView>>, AppError> {
    let view = service::complete_upload(
        &state.db,
        &current_user,
        upload_id,
        &state.config.upload_dir,
    )
    .await
    .map_err(map_error)?;
    Ok(success(view))
}

pub async fn abort_upload(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(upload_id): Path<Uuid>,
) -> Result<Json<ApiEnvelope<serde_json::Value>>, AppError> {
    service::abort_upload(
        &state.db,
        &current_user,
        upload_id,
        &state.config.upload_dir,
    )
    .await
    .map_err(map_error)?;
    Ok(success(serde_json::json!({ "message": "上传会话已清理" })))
}

/// 附件内容公开读取:<img> 无法携带 Authorization,以不可猜的 object_key 作为访问凭证。
/// 图片 inline 供预览;其他类型强制 attachment 下载,防 HTML/SVG 同源脚本执行。
/// 流式输出并支持单区间 Range(206/416),供断点续传与分段下载。
pub async fn content(
    State(state): State<AppState>,
    Path(object_key): Path<String>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let content = service::open_content(&state.db, &object_key, &state.config.upload_dir)
        .await
        .map_err(map_error)?;
    let disposition = if content.mime_type.starts_with("image/") {
        "inline".to_owned()
    } else {
        // RFC 5987:中文文件名走 filename*,同时给 ASCII 兜底 filename。
        let encoded = percent_encoding(content.file_name.as_bytes());
        format!("attachment; filename=\"attachment\"; filename*=UTF-8''{encoded}")
    };
    // If-Range 命中(或未带)才应用 Range;不匹配说明资源可能已变,回退 200 全量。
    let if_range_matches = headers
        .get(header::IF_RANGE)
        .and_then(|value| value.to_str().ok())
        .is_none_or(|value| value.trim() == content.etag);
    let range = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .filter(|_| if_range_matches)
        .map(|value| service::parse_range_header(value, content.byte_size));
    let size = content.byte_size;
    let builder = Response::builder()
        .header(header::CONTENT_TYPE, content.mime_type)
        .header(header::CONTENT_DISPOSITION, disposition)
        .header(
            header::CACHE_CONTROL,
            "private, max-age=31536000, immutable",
        )
        .header("X-Content-Type-Options", "nosniff")
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::ETAG, content.etag);
    match range {
        Some(service::RangeParse::Unsatisfiable) => builder
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(header::CONTENT_RANGE, format!("bytes */{size}"))
            .body(Body::empty())
            .map_err(|_| AppError::internal("附件读取暂时不可用")),
        Some(service::RangeParse::Satisfiable(start, end)) => {
            let mut file = content.file;
            file.seek(SeekFrom::Start(start))
                .await
                .map_err(|error| AppError::internal(format!("附件读取暂时不可用：{error}")))?;
            let length = end - start + 1;
            let stream = ReaderStream::with_capacity(file.take(length), 64 * 1024);
            builder
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{size}"))
                .header(header::CONTENT_LENGTH, length.to_string())
                .body(Body::from_stream(stream))
                .map_err(|_| AppError::internal("附件读取暂时不可用"))
        }
        _ => {
            let stream = ReaderStream::with_capacity(content.file, 256 * 1024);
            builder
                .status(StatusCode::OK)
                .header(header::CONTENT_LENGTH, size.to_string())
                .body(Body::from_stream(stream))
                .map_err(|_| AppError::internal("附件读取暂时不可用"))
        }
    }
}

/// 百分号编码(UTF-8 字节),不依赖额外 crate。
fn percent_encoding(value: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(value.len());
    for byte in value {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(*byte as char)
            }
            _ => {
                out.push('%');
                out.push(HEX[(byte >> 4) as usize] as char);
                out.push(HEX[(byte & 0x0f) as usize] as char);
            }
        }
    }
    out
}

#[derive(serde::Deserialize)]
pub struct DeleteAttachmentRequest {
    pub reason: Option<String>,
}

pub async fn delete(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(request): Json<DeleteAttachmentRequest>,
) -> Result<Json<ApiEnvelope<serde_json::Value>>, AppError> {
    service::delete(&state.db, &current_user, id, request.reason)
        .await
        .map_err(map_error)?;
    Ok(success(serde_json::json!({ "message": "附件已删除" })))
}
