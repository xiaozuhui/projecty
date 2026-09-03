use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{header, StatusCode},
    response::Response,
    Json,
};
use uuid::Uuid;

use crate::{
    http::{
        error::{success, ApiEnvelope, AppError},
        extractors::CurrentUser,
    },
    modules::attachments::service::{self, AttachmentError, AttachmentView},
    state::AppState,
};

fn map_error(error: AttachmentError) -> AppError {
    match error {
        AttachmentError::NotFound => AppError {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: "附件不存在".to_owned(),
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

pub async fn upload(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(task_key): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<ApiEnvelope<AttachmentView>>, AppError> {
    let mut file: Option<(Option<String>, Option<String>, Vec<u8>)> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| AppError::bad_request(format!("文件上传失败：{error}")))?
    {
        if field.name() == Some("file") {
            let name = field.file_name().map(|value| value.to_owned());
            let content_type = field.content_type().map(|value| value.to_owned());
            let bytes = field
                .bytes()
                .await
                .map_err(|error| AppError::bad_request(format!("文件读取失败：{error}")))?;
            file = Some((name, content_type, bytes.to_vec()));
            break;
        }
    }
    let (name, content_type, bytes) =
        file.ok_or_else(|| AppError::bad_request("需要上传名为 file 的文件"))?;
    let view = service::upload(
        &state.db,
        &current_user,
        &task_key,
        service::IncomingFile {
            name,
            content_type,
            bytes,
        },
        &state.config.upload_dir,
        state.config.upload_max_bytes,
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

/// 附件内容公开读取:<img> 无法携带 Authorization,以不可猜的 object_key 作为访问凭证。
/// 图片 inline 供预览;其他类型强制 attachment 下载,防 HTML/SVG 同源脚本执行。
pub async fn content(
    State(state): State<AppState>,
    Path(object_key): Path<String>,
) -> Result<Response, AppError> {
    let (bytes, mime_type, file_name) =
        service::read_content(&state.db, &object_key, &state.config.upload_dir)
            .await
            .map_err(map_error)?;
    let disposition = if mime_type.starts_with("image/") {
        "inline".to_owned()
    } else {
        // RFC 5987:中文文件名走 filename*,同时给 ASCII 兜底 filename。
        let encoded = percent_encoding(file_name.as_bytes());
        format!(
            "attachment; filename=\"attachment\"; filename*=UTF-8''{encoded}"
        )
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime_type)
        .header(header::CONTENT_DISPOSITION, disposition)
        .header(
            header::CACHE_CONTROL,
            "private, max-age=31536000, immutable",
        )
        .header("X-Content-Type-Options", "nosniff")
        .body(Body::from(bytes))
        .map_err(|_| AppError::internal("附件读取暂时不可用"))
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
