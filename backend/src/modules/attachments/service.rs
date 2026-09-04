//! 任务附件:图片与普通文件写入本地磁盘、按不可猜的 object_key 公开读取、软删除与审计。
//! 大文件走分片上传(init/分片/complete),按文件指纹断点续传;下载流式输出并支持 Range 分段。
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};
use projecty_entity::{
    attachment_upload_chunks, attachment_upload_sessions, operation_logs, task_attachments, tasks,
    users,
};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, Set, TransactionTrait,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

use crate::{http::extractors::CurrentUser, modules::tasks::service::user_can_read_project};

/// 会话最长保留时长:超时无活动的会话在下次 init 时连同分片一起清理。
const SESSION_STALE_AFTER_HOURS: i64 = 24;

#[derive(Debug, thiserror::Error)]
pub enum AttachmentError {
    #[error("附件或任务不存在")]
    NotFound,
    #[error("上传会话不存在或已过期")]
    SessionNotFound,
    #[error("没有当前任务的操作权限")]
    Forbidden,
    #[error("请求参数无效：{0}")]
    InvalidInput(String),
    #[error("数据库错误：{0}")]
    Database(#[from] sea_orm::DbErr),
    #[error("附件存储读写失败：{0}")]
    Io(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct AttachmentView {
    pub id: Uuid,
    pub task_id: Uuid,
    pub comment_id: Option<Uuid>,
    pub file_name: String,
    pub object_key: String,
    pub mime_type: String,
    pub byte_size: i64,
    pub uploader_id: Uuid,
    pub uploader_name: String,
    pub url: String,
    pub created_at: DateTime<Utc>,
}

/// 分片上传会话视图:received_chunks 升序,客户端据此跳过已传分片完成断点续传。
#[derive(Debug, Clone, Serialize)]
pub struct UploadSessionView {
    pub upload_id: Uuid,
    pub status: String,
    pub chunk_size: i64,
    pub total_bytes: i64,
    pub total_chunks: i64,
    pub received_chunks: Vec<i64>,
}

/// sha2 0.11 的摘要不再实现 LowerHex,手动转十六进制(与 auth 模块一致;handler 流式上传也要用)。
pub fn hex_digest(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut hex, byte| {
            hex.push_str(&format!("{byte:02x}"));
            hex
        })
}

fn io_error(error: std::io::Error) -> AttachmentError {
    AttachmentError::Io(error.to_string())
}

/// 按 magic bytes 识别图片真实类型,防止改扩展名上传非图片内容。
fn sniff_image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

fn extension_for(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "bin",
    }
}

/// 通用文件的存储扩展名:取原始文件名最后一段扩展,白名单 [A-Za-z0-9]{1,12},不合法落 bin。
fn safe_extension(file_name: Option<&str>) -> String {
    let raw = file_name
        .and_then(|name| name.rsplit_once('.'))
        .map(|(_, ext)| ext)
        .unwrap_or("");
    if !raw.is_empty() && raw.len() <= 12 && raw.chars().all(|c| c.is_ascii_alphanumeric()) {
        raw.to_ascii_lowercase()
    } else {
        "bin".to_owned()
    }
}

/// 声明 content_type 规范化:截掉 ; 参数,空白/过长/伪装图片类型兜底 octet-stream。
///
/// 普通文件的 MIME 来自客户端,不能据此把未嗅探出的内容标成 image/*;
/// 否则 SVG 或伪装成 image/png 的脚本文件会被 content 接口 inline 返回。
fn normalize_declared_mime(content_type: Option<&str>) -> String {
    let mime = content_type
        .map(|value| value.split(';').next().unwrap_or("").trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("application/octet-stream");
    if mime.len() > 64 || mime.to_ascii_lowercase().starts_with("image/") {
        "application/octet-stream".to_owned()
    } else {
        mime.to_owned()
    }
}

/// 展示用文件名:只保留最终路径段,截断到 200 字符。
fn sanitize_file_name(name: &str) -> String {
    let base = Path::new(name)
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "image".to_owned());
    let trimmed = base.trim();
    if trimmed.is_empty() {
        "image".to_owned()
    } else {
        trimmed.chars().take(200).collect()
    }
}

/// object_key 仅允许 UUIDv4 + 白名单扩展名的字符集,拒绝任何路径拼接输入。
pub fn valid_object_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 64
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
}

/// 客户端文件指纹(断点续传匹配键):非空、≤128、可打印 ASCII 且不含空格。
fn valid_client_file_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// 图片按嗅探结果定型(防改扩展名伪装);其他文件信任声明的 content_type,
/// 扩展名从原始文件名提取并白名单化,object_key 字符集不受影响。
fn resolve_mime_and_ext(
    first_bytes: &[u8],
    declared: Option<&str>,
    file_name: Option<&str>,
) -> Result<(String, String), AttachmentError> {
    match sniff_image_mime(first_bytes) {
        Some(sniffed) => {
            if let Some(declared) = declared {
                let declared = declared.split(';').next().unwrap_or("").trim();
                if !declared.is_empty() && declared != sniffed {
                    return Err(AttachmentError::InvalidInput(format!(
                        "文件内容与声明类型不符：声明 {declared}，实际 {sniffed}"
                    )));
                }
            }
            Ok((sniffed.to_owned(), extension_for(sniffed).to_owned()))
        }
        None => Ok((normalize_declared_mime(declared), safe_extension(file_name))),
    }
}

fn view_from(model: task_attachments::Model, uploader_name: String) -> AttachmentView {
    AttachmentView {
        url: format!("/attachments/{}/content", model.object_key),
        id: model.id,
        task_id: model.task_id,
        comment_id: model.comment_id,
        file_name: model.file_name,
        object_key: model.object_key,
        mime_type: model.mime_type,
        byte_size: model.byte_size,
        uploader_id: model.uploader_id,
        uploader_name,
        created_at: model.created_at,
    }
}

async fn find_task(
    db: &DatabaseConnection,
    task_key: &str,
) -> Result<tasks::Model, AttachmentError> {
    tasks::Entity::find()
        .filter(tasks::Column::TaskKey.eq(task_key))
        .filter(tasks::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(AttachmentError::NotFound)
}

fn attachment_audit(
    current_user: &CurrentUser,
    task: &tasks::Model,
    inserted: &task_attachments::Model,
    now: DateTime<Utc>,
) -> operation_logs::ActiveModel {
    operation_logs::ActiveModel {
        id: Set(Uuid::now_v7()),
        actor_user_id: Set(current_user.user_id),
        module: Set("attachment".to_owned()),
        action: Set("upload".to_owned()),
        project_id: Set(Some(task.project_id)),
        task_id: Set(Some(task.id)),
        target_type: Set("attachment".to_owned()),
        target_id: Set(Some(inserted.id)),
        summary: Set(format!("上传任务附件：{}", task.task_key)),
        diff: Set(Some(serde_json::json!({
            "file_name": inserted.file_name.clone(),
            "mime_type": inserted.mime_type.clone(),
            "byte_size": inserted.byte_size,
        }))),
        snapshot: Set(None),
        created_at: Set(now),
    }
}

async fn attachment_view_by_id(
    db: &DatabaseConnection,
    attachment_id: Uuid,
) -> Result<AttachmentView, AttachmentError> {
    let attachment = task_attachments::Entity::find_by_id(attachment_id)
        .filter(task_attachments::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(AttachmentError::NotFound)?;
    let uploader = users::Entity::find_by_id(attachment.uploader_id)
        .one(db)
        .await?
        .ok_or(AttachmentError::NotFound)?;
    Ok(view_from(attachment, uploader.display_name))
}

fn staging_root(upload_dir: &Path) -> PathBuf {
    upload_dir.join(".staging")
}

/// legacy 单发路径的暂存文件路径:handler 流式写入,校验通过后由 upload rename 到最终位置。
pub fn legacy_staged_path(upload_dir: &Path) -> PathBuf {
    staging_root(upload_dir).join(format!("legacy-{}.part", Uuid::new_v4().simple()))
}

fn staging_dir(upload_dir: &Path, upload_id: Uuid) -> PathBuf {
    staging_root(upload_dir).join(upload_id.to_string())
}

fn part_path(dir: &Path, index: i32) -> PathBuf {
    dir.join(format!("{index:06}.part"))
}

fn total_chunks_of(session: &attachment_upload_sessions::Model) -> i64 {
    // 两边恒为正数,向上取整手写避免依赖 div_ceil 的版本门槛。
    (session.total_bytes + session.chunk_size - 1) / session.chunk_size
}

fn chunk_expected_size(session: &attachment_upload_sessions::Model, index: i64) -> i64 {
    session
        .chunk_size
        .min(session.total_bytes - index * session.chunk_size)
}

/// 暂存目录清理:目录不存在视为已清理,其余仅记录不影响主流程。
async fn cleanup_staging(upload_dir: &Path, upload_id: Uuid) {
    if let Err(error) = tokio::fs::remove_dir_all(staging_dir(upload_dir, upload_id)).await {
        if error.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!(%error, "remove upload staging failed");
        }
    }
}

/// 会话作废:状态改 aborted 并删除暂存目录(行保留,由过期清理统一回收)。
async fn discard_session(
    db: &DatabaseConnection,
    session: &attachment_upload_sessions::Model,
    upload_dir: &Path,
) {
    let mut active: attachment_upload_sessions::ActiveModel = session.clone().into();
    active.status = Set("aborted".to_owned());
    active.updated_at = Set(Utc::now());
    if let Err(error) = active.update(db).await {
        tracing::warn!(?error, "discard upload session failed");
    }
    cleanup_staging(upload_dir, session.id).await;
}

/// 仅活动时间刷新:失败不影响分片上传本身。
async fn touch_session(db: &DatabaseConnection, upload_id: Uuid) {
    if let Err(error) = attachment_upload_sessions::Entity::update_many()
        .col_expr(
            attachment_upload_sessions::Column::UpdatedAt,
            Expr::value(Utc::now()),
        )
        .filter(attachment_upload_sessions::Column::Id.eq(upload_id))
        .exec(db)
        .await
    {
        tracing::warn!(?error, "touch upload session failed");
    }
}

async fn load_owned_session(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    upload_id: Uuid,
) -> Result<attachment_upload_sessions::Model, AttachmentError> {
    let session = attachment_upload_sessions::Entity::find_by_id(upload_id)
        .one(db)
        .await?
        .ok_or(AttachmentError::SessionNotFound)?;
    // 他人会话不区分不存在与无权限,统一按不存在处理,避免会话 id 探测。
    if session.uploader_id != current_user.user_id {
        return Err(AttachmentError::SessionNotFound);
    }
    Ok(session)
}

async fn session_view(
    db: &DatabaseConnection,
    session: attachment_upload_sessions::Model,
) -> Result<UploadSessionView, AttachmentError> {
    let received = attachment_upload_chunks::Entity::find()
        .filter(attachment_upload_chunks::Column::SessionId.eq(session.id))
        .order_by_asc(attachment_upload_chunks::Column::ChunkIndex)
        .all(db)
        .await?
        .into_iter()
        .map(|row| row.chunk_index as i64)
        .collect();
    let total_chunks = total_chunks_of(&session);
    Ok(UploadSessionView {
        upload_id: session.id,
        status: session.status,
        chunk_size: session.chunk_size,
        total_bytes: session.total_bytes,
        total_chunks,
        received_chunks: received,
    })
}

/// 机会式清理:超过 24h 无活动的会话删除暂存目录与全部行,避免磁盘与表无限膨胀。
async fn purge_stale_sessions(db: &DatabaseConnection, upload_dir: &Path) {
    let cutoff = Utc::now() - chrono::Duration::hours(SESSION_STALE_AFTER_HOURS);
    let stale = match attachment_upload_sessions::Entity::find()
        .filter(attachment_upload_sessions::Column::UpdatedAt.lt(cutoff))
        .all(db)
        .await
    {
        Ok(rows) if !rows.is_empty() => rows,
        Ok(_) => return,
        Err(error) => {
            tracing::warn!(?error, "query stale upload sessions failed");
            return;
        }
    };
    let ids: Vec<Uuid> = stale.iter().map(|session| session.id).collect();
    for session in &stale {
        cleanup_staging(upload_dir, session.id).await;
    }
    if let Err(error) = attachment_upload_chunks::Entity::delete_many()
        .filter(attachment_upload_chunks::Column::SessionId.is_in(ids.clone()))
        .exec(db)
        .await
    {
        tracing::warn!(?error, "delete stale upload chunks failed");
    }
    if let Err(error) = attachment_upload_sessions::Entity::delete_many()
        .filter(attachment_upload_sessions::Column::Id.is_in(ids))
        .exec(db)
        .await
    {
        tracing::warn!(?error, "delete stale upload sessions failed");
    }
}

// ---- 单发上传(legacy,已流式暂存到磁盘) ----

/// 已暂存到磁盘的上传内容:handler 流式写入暂存文件,校验通过后 rename 到最终位置。
pub struct IncomingStaged {
    pub staged_path: PathBuf,
    pub byte_size: u64,
    pub first_bytes: Vec<u8>,
    pub sha256_hex: String,
    pub name: Option<String>,
    pub content_type: Option<String>,
}

pub async fn upload(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    task_key: &str,
    file: IncomingStaged,
    upload_dir: &Path,
    max_bytes: usize,
) -> Result<AttachmentView, AttachmentError> {
    let result = upload_inner(db, current_user, task_key, &file, upload_dir, max_bytes).await;
    if result.is_err() {
        // 校验失败时暂存文件已无用途,尽力清理避免堆积。
        let _ = tokio::fs::remove_file(&file.staged_path).await;
    }
    result
}

async fn upload_inner(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    task_key: &str,
    file: &IncomingStaged,
    upload_dir: &Path,
    max_bytes: usize,
) -> Result<AttachmentView, AttachmentError> {
    let task = find_task(db, task_key).await?;
    if !user_can_read_project(db, current_user, task.project_id).await? {
        return Err(AttachmentError::Forbidden);
    }
    if file.byte_size == 0 {
        return Err(AttachmentError::InvalidInput("上传文件不能为空".to_owned()));
    }
    if file.byte_size > max_bytes as u64 {
        return Err(AttachmentError::InvalidInput(format!(
            "文件大小不能超过 {} MB",
            max_bytes / 1024 / 1024
        )));
    }
    let (mime, extension) = resolve_mime_and_ext(
        &file.first_bytes,
        file.content_type.as_deref(),
        file.name.as_deref(),
    )?;
    let object_key = format!("{}.{}", Uuid::new_v4(), extension);
    let display_name = sanitize_file_name(file.name.as_deref().unwrap_or("file"));
    let now = Utc::now();
    let attachment = task_attachments::ActiveModel {
        id: Set(Uuid::now_v7()),
        task_id: Set(task.id),
        comment_id: Set(None),
        uploader_id: Set(current_user.user_id),
        file_name: Set(display_name),
        object_key: Set(object_key.clone()),
        mime_type: Set(mime.clone()),
        byte_size: Set(file.byte_size as i64),
        sha256: Set(Some(file.sha256_hex.clone())),
        created_at: Set(now),
        deleted_at: Set(None),
    };
    let txn = db.begin().await?;
    let inserted = attachment.insert(&txn).await?;
    attachment_audit(current_user, &task, &inserted, now)
        .insert(&txn)
        .await?;
    // 事务内先落盘再提交:写盘失败时数据库行一并回滚,不留指向缺失文件的记录。
    tokio::fs::create_dir_all(upload_dir)
        .await
        .map_err(io_error)?;
    tokio::fs::rename(&file.staged_path, upload_dir.join(&object_key))
        .await
        .map_err(io_error)?;
    txn.commit().await?;
    let uploader = users::Entity::find_by_id(current_user.user_id)
        .one(db)
        .await?
        .ok_or(AttachmentError::NotFound)?;
    Ok(view_from(inserted, uploader.display_name))
}

// ---- 分片上传会话 ----

pub struct InitUploadInput {
    pub file_name: String,
    pub mime_type: Option<String>,
    pub total_bytes: i64,
    pub client_file_key: String,
    pub client_sha256: Option<String>,
}

/// 建立上传会话。同 uploader+task+文件指纹的 active 会话直接复用(断点续传),
/// 文件大小变化说明换了内容,作废旧会话后新建。chunk_size 由服务端统一下发。
pub async fn init_upload(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    task_key: &str,
    input: InitUploadInput,
    upload_dir: &Path,
    max_bytes: usize,
    chunk_bytes: usize,
) -> Result<UploadSessionView, AttachmentError> {
    let task = find_task(db, task_key).await?;
    if !user_can_read_project(db, current_user, task.project_id).await? {
        return Err(AttachmentError::Forbidden);
    }
    if input.total_bytes < 1 {
        return Err(AttachmentError::InvalidInput("上传文件不能为空".to_owned()));
    }
    if input.total_bytes > max_bytes as i64 {
        return Err(AttachmentError::InvalidInput(format!(
            "文件大小不能超过 {} MB",
            max_bytes / 1024 / 1024
        )));
    }
    if !valid_client_file_key(&input.client_file_key) {
        return Err(AttachmentError::InvalidInput("文件指纹格式无效".to_owned()));
    }
    if input.file_name.trim().is_empty() {
        return Err(AttachmentError::InvalidInput("文件名不能为空".to_owned()));
    }
    if let Some(sha256) = input.client_sha256.as_deref() {
        if !valid_sha256(sha256) {
            return Err(AttachmentError::InvalidInput(
                "文件校验值格式无效".to_owned(),
            ));
        }
    }
    purge_stale_sessions(db, upload_dir).await;
    let existing = attachment_upload_sessions::Entity::find()
        .filter(attachment_upload_sessions::Column::UploaderId.eq(current_user.user_id))
        .filter(attachment_upload_sessions::Column::TaskId.eq(task.id))
        .filter(
            attachment_upload_sessions::Column::ClientFileKey.eq(input.client_file_key.as_str()),
        )
        .filter(attachment_upload_sessions::Column::Status.eq("active"))
        .order_by_desc(attachment_upload_sessions::Column::CreatedAt)
        .one(db)
        .await?;
    if let Some(session) = existing {
        if session.total_bytes == input.total_bytes {
            return session_view(db, session).await;
        }
        discard_session(db, &session, upload_dir).await;
    }
    let now = Utc::now();
    let session = attachment_upload_sessions::ActiveModel {
        id: Set(Uuid::now_v7()),
        task_id: Set(task.id),
        uploader_id: Set(current_user.user_id),
        client_file_key: Set(input.client_file_key),
        file_name: Set(input.file_name.chars().take(200).collect()),
        declared_mime: Set(input.mime_type),
        total_bytes: Set(input.total_bytes),
        chunk_size: Set(chunk_bytes as i64),
        status: Set("active".to_owned()),
        attachment_id: Set(None),
        client_sha256: Set(input.client_sha256),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(db)
    .await?;
    tokio::fs::create_dir_all(staging_dir(upload_dir, session.id))
        .await
        .map_err(io_error)?;
    session_view(db, session).await
}

/// 接收单个分片:校验长度与可选 sha256 后落盘,再记回执。
/// 文件先写、回执后入账:中途崩溃时 resume 报缺该分片,客户端重传即可自愈。
pub async fn upload_chunk(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    upload_id: Uuid,
    index: i32,
    bytes: &[u8],
    checksum: Option<&str>,
    upload_dir: &Path,
) -> Result<UploadSessionView, AttachmentError> {
    let session = load_owned_session(db, current_user, upload_id).await?;
    if session.status != "active" {
        return Err(AttachmentError::SessionNotFound);
    }
    if index < 0 || index as i64 >= total_chunks_of(&session) {
        return Err(AttachmentError::InvalidInput("分片序号超出范围".to_owned()));
    }
    if bytes.len() as i64 != chunk_expected_size(&session, index as i64) {
        return Err(AttachmentError::InvalidInput(
            "分片大小与会话不一致".to_owned(),
        ));
    }
    if let Some(checksum) = checksum {
        if !valid_sha256(checksum) || hex_digest(&Sha256::digest(bytes)) != checksum {
            return Err(AttachmentError::InvalidInput(
                "分片校验失败，请重试该分片".to_owned(),
            ));
        }
    }
    let dir = staging_dir(upload_dir, session.id);
    tokio::fs::create_dir_all(&dir).await.map_err(io_error)?;
    // 先写临时文件再 rename:并发重传同一分片也不会留下半截内容。
    let temp = dir.join(format!("{index:06}.part.{}.tmp", Uuid::new_v4().simple()));
    let mut writer = tokio::fs::File::create(&temp).await.map_err(io_error)?;
    writer.write_all(bytes).await.map_err(io_error)?;
    writer.flush().await.map_err(io_error)?;
    drop(writer);
    tokio::fs::rename(&temp, part_path(&dir, index))
        .await
        .map_err(io_error)?;
    let received = attachment_upload_chunks::Entity::find_by_id((session.id, index))
        .one(db)
        .await?;
    if received.is_none() {
        attachment_upload_chunks::ActiveModel {
            session_id: Set(session.id),
            chunk_index: Set(index),
            received_at: Set(Utc::now()),
        }
        .insert(db)
        .await?;
    }
    touch_session(db, session.id).await;
    session_view(db, session).await
}

/// 查询会话状态:断点续传时客户端据此获取已收分片列表。
pub async fn upload_session_state(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    upload_id: Uuid,
) -> Result<UploadSessionView, AttachmentError> {
    let session = load_owned_session(db, current_user, upload_id).await?;
    session_view(db, session).await
}

/// 完成上传:以磁盘为准核对全部分片,拼接为完整文件后落库 + 审计。
/// 条件认领会话保证幂等:并发/重复 complete 只有一个赢家,输者返回赢家的附件。
pub async fn complete_upload(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    upload_id: Uuid,
    upload_dir: &Path,
) -> Result<AttachmentView, AttachmentError> {
    let session = load_owned_session(db, current_user, upload_id).await?;
    if session.status == "completed" {
        return match session.attachment_id {
            Some(attachment_id) => attachment_view_by_id(db, attachment_id).await,
            None => Err(AttachmentError::SessionNotFound),
        };
    }
    if session.status != "active" {
        return Err(AttachmentError::InvalidInput("上传会话已中止".to_owned()));
    }
    // init 之后任务可能被删或权限被收,complete 时重新校验。
    let task = tasks::Entity::find_by_id(session.task_id)
        .filter(tasks::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(AttachmentError::NotFound)?;
    if !user_can_read_project(db, current_user, task.project_id).await? {
        return Err(AttachmentError::Forbidden);
    }
    let dir = staging_dir(upload_dir, session.id);
    let total_chunks = total_chunks_of(&session);
    // 逐片核对磁盘(而非仅看回执):崩溃产生的“有行无文件”也能被发现。
    let mut missing = Vec::new();
    for index in 0..total_chunks {
        let size = match tokio::fs::metadata(part_path(&dir, index as i32)).await {
            Ok(meta) => meta.len() as i64,
            Err(_) => -1,
        };
        if size != chunk_expected_size(&session, index) {
            missing.push(index);
        }
    }
    if !missing.is_empty() {
        return Err(AttachmentError::InvalidInput(format!(
            "分片缺失或不完整：{missing:?}"
        )));
    }
    // 拼接分片,同时计算整文件 sha256 并保留头部字节用于 MIME 嗅探。
    let assembled = dir.join("assembled");
    let mut hasher = Sha256::new();
    let mut head: Vec<u8> = Vec::new();
    let mut writer = tokio::fs::File::create(&assembled)
        .await
        .map_err(io_error)?;
    let mut buffer = vec![0u8; 256 * 1024];
    for index in 0..total_chunks {
        let mut reader = tokio::fs::File::open(part_path(&dir, index as i32))
            .await
            .map_err(io_error)?;
        loop {
            let read = reader.read(&mut buffer).await.map_err(io_error)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            if head.len() < 32 {
                let take = (32 - head.len()).min(read);
                head.extend_from_slice(&buffer[..take]);
            }
            writer.write_all(&buffer[..read]).await.map_err(io_error)?;
        }
    }
    writer.flush().await.map_err(io_error)?;
    drop(writer);
    let sha256_hex = hex_digest(&hasher.finalize());
    if session
        .client_sha256
        .as_deref()
        .is_some_and(|expected| expected != sha256_hex)
    {
        // 整文件校验失败:指纹碰撞或分片错乱,作废会话让客户端重新上传。
        discard_session(db, &session, upload_dir).await;
        return Err(AttachmentError::InvalidInput(
            "文件校验失败，请重新上传".to_owned(),
        ));
    }
    let (mime, extension) = resolve_mime_and_ext(
        &head,
        session.declared_mime.as_deref(),
        Some(&session.file_name),
    )?;
    let object_key = format!("{}.{}", Uuid::new_v4(), extension);
    let display_name = sanitize_file_name(&session.file_name);
    let now = Utc::now();
    let txn = db.begin().await?;
    let claim = attachment_upload_sessions::Entity::update_many()
        .col_expr(
            attachment_upload_sessions::Column::Status,
            Expr::value("completed"),
        )
        .filter(attachment_upload_sessions::Column::Id.eq(session.id))
        .filter(attachment_upload_sessions::Column::Status.eq("active"))
        .exec(&txn)
        .await?;
    if claim.rows_affected == 0 {
        txn.rollback().await?;
        let refreshed = attachment_upload_sessions::Entity::find_by_id(session.id)
            .one(db)
            .await?
            .ok_or(AttachmentError::SessionNotFound)?;
        return match refreshed.attachment_id {
            Some(attachment_id) => attachment_view_by_id(db, attachment_id).await,
            None => Err(AttachmentError::SessionNotFound),
        };
    }
    let attachment = task_attachments::ActiveModel {
        id: Set(Uuid::now_v7()),
        task_id: Set(task.id),
        comment_id: Set(None),
        uploader_id: Set(current_user.user_id),
        file_name: Set(display_name),
        object_key: Set(object_key.clone()),
        mime_type: Set(mime.clone()),
        byte_size: Set(session.total_bytes),
        sha256: Set(Some(sha256_hex)),
        created_at: Set(now),
        deleted_at: Set(None),
    };
    let inserted = attachment.insert(&txn).await?;
    attachment_audit(current_user, &task, &inserted, now)
        .insert(&txn)
        .await?;
    attachment_upload_sessions::Entity::update_many()
        .col_expr(
            attachment_upload_sessions::Column::AttachmentId,
            Expr::value(inserted.id),
        )
        .filter(attachment_upload_sessions::Column::Id.eq(session.id))
        .exec(&txn)
        .await?;
    // 事务内先落盘再提交:写盘失败时数据库行一并回滚,不留指向缺失文件的记录。
    tokio::fs::create_dir_all(upload_dir)
        .await
        .map_err(io_error)?;
    tokio::fs::rename(&assembled, upload_dir.join(&object_key))
        .await
        .map_err(io_error)?;
    txn.commit().await?;
    cleanup_staging(upload_dir, session.id).await;
    let uploader = users::Entity::find_by_id(current_user.user_id)
        .one(db)
        .await?
        .ok_or(AttachmentError::NotFound)?;
    Ok(view_from(inserted, uploader.display_name))
}

/// 放弃上传:作废会话并清理暂存分片。
pub async fn abort_upload(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    upload_id: Uuid,
    upload_dir: &Path,
) -> Result<(), AttachmentError> {
    let session = load_owned_session(db, current_user, upload_id).await?;
    if session.status == "active" {
        discard_session(db, &session, upload_dir).await;
    }
    Ok(())
}

// ---- 读取与分段下载 ----

/// 公开读取入口返回的文件元数据 + 已打开的句柄,供 handler 组装 200/206 响应。
pub struct ContentFile {
    pub byte_size: u64,
    pub mime_type: String,
    pub file_name: String,
    pub etag: String,
    pub file: tokio::fs::File,
}

/// 公开读取入口:仅凭库中存在且未软删的 object_key 命中。
/// etag 优先用内容 sha256,旧数据无哈希时退化为 key+size(仍是强校验值,足够 If-Range 使用)。
pub async fn open_content(
    db: &DatabaseConnection,
    object_key: &str,
    upload_dir: &Path,
) -> Result<ContentFile, AttachmentError> {
    if !valid_object_key(object_key) {
        return Err(AttachmentError::NotFound);
    }
    let attachment = task_attachments::Entity::find()
        .filter(task_attachments::Column::ObjectKey.eq(object_key))
        .filter(task_attachments::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(AttachmentError::NotFound)?;
    let file = tokio::fs::File::open(upload_dir.join(object_key))
        .await
        .map_err(io_error)?;
    let byte_size = file.metadata().await.map_err(io_error)?.len();
    let etag = match attachment.sha256.as_deref() {
        Some(sha256) => format!("\"{sha256}\""),
        None => format!("\"{}-{}\"", attachment.object_key, attachment.byte_size),
    };
    Ok(ContentFile {
        byte_size,
        mime_type: attachment.mime_type,
        file_name: attachment.file_name,
        etag,
        file,
    })
}

#[derive(Debug, PartialEq, Eq)]
pub enum RangeParse {
    Satisfiable(u64, u64),
    Unsatisfiable,
    Ignore,
}

/// 单区间 Range 解析:多区间/畸形输入返回 Ignore 走 200 全量;
/// 起点越界或空后缀(bytes=-0)返回 Unsatisfiable 走 416。end 一律夹到 size-1。
pub fn parse_range_header(value: &str, size: u64) -> RangeParse {
    let Some(spec) = value.trim().strip_prefix("bytes=") else {
        return RangeParse::Ignore;
    };
    if spec.contains(',') {
        return RangeParse::Ignore;
    }
    let Some((start, end)) = spec.split_once('-') else {
        return RangeParse::Ignore;
    };
    let (start, end) = (start.trim(), end.trim());
    if start.is_empty() {
        // 后缀区间 bytes=-n:取最后 n 字节;n 超过文件大小视为整文件。
        let Ok(suffix) = end.parse::<u64>() else {
            return RangeParse::Ignore;
        };
        if size == 0 || suffix == 0 {
            return RangeParse::Unsatisfiable;
        }
        let suffix = suffix.min(size);
        return RangeParse::Satisfiable(size - suffix, size - 1);
    }
    let Ok(start) = start.parse::<u64>() else {
        return RangeParse::Ignore;
    };
    if start >= size {
        return RangeParse::Unsatisfiable;
    }
    let end = if end.is_empty() {
        size - 1
    } else {
        match end.parse::<u64>() {
            Ok(end) if end < start => return RangeParse::Ignore,
            Ok(end) => end.min(size - 1),
            Err(_) => return RangeParse::Ignore,
        }
    };
    RangeParse::Satisfiable(start, end)
}

pub async fn list(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    task_key: &str,
) -> Result<Vec<AttachmentView>, AttachmentError> {
    let task = find_task(db, task_key).await?;
    if !user_can_read_project(db, current_user, task.project_id).await? {
        return Err(AttachmentError::Forbidden);
    }
    let rows = task_attachments::Entity::find()
        .filter(task_attachments::Column::TaskId.eq(task.id))
        .filter(task_attachments::Column::DeletedAt.is_null())
        .order_by_asc(task_attachments::Column::CreatedAt)
        .find_also_related(users::Entity)
        .all(db)
        .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(attachment, uploader)| {
            uploader.map(|user| view_from(attachment, user.display_name))
        })
        .collect())
}

pub async fn delete(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    id: Uuid,
    reason: Option<String>,
) -> Result<(), AttachmentError> {
    let attachment = task_attachments::Entity::find_by_id(id)
        .filter(task_attachments::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(AttachmentError::NotFound)?;
    let task = tasks::Entity::find_by_id(attachment.task_id)
        .one(db)
        .await?
        .ok_or(AttachmentError::NotFound)?;
    if !user_can_read_project(db, current_user, task.project_id).await? {
        return Err(AttachmentError::Forbidden);
    }
    if attachment.uploader_id != current_user.user_id && !current_user.system_role.is_super_admin()
    {
        return Err(AttachmentError::Forbidden);
    }
    let now = Utc::now();
    let mut active: task_attachments::ActiveModel = attachment.clone().into();
    active.deleted_at = Set(Some(now));
    let txn = db.begin().await?;
    active.update(&txn).await?;
    operation_logs::ActiveModel {
        id: Set(Uuid::now_v7()),
        actor_user_id: Set(current_user.user_id),
        module: Set("attachment".to_owned()),
        action: Set("delete".to_owned()),
        project_id: Set(Some(task.project_id)),
        task_id: Set(Some(task.id)),
        target_type: Set("attachment".to_owned()),
        target_id: Set(Some(attachment.id)),
        summary: Set(format!("逻辑删除任务附件：{}", task.task_key)),
        diff: Set(Some(serde_json::json!({ "reason": reason }))),
        snapshot: Set(None),
        created_at: Set(now),
    }
    .insert(&txn)
    .await?;
    txn.commit().await?;
    Ok(())
}

/// 评论创建时关联附件:仅允许本人上传、同任务且尚未关联的附件。
pub async fn link_to_comment<C: ConnectionTrait + Send + Sync>(
    conn: &C,
    actor_user_id: Uuid,
    task_id: Uuid,
    comment_id: Uuid,
    attachment_ids: &[Uuid],
) -> Result<(), AttachmentError> {
    for id in attachment_ids {
        let attachment = task_attachments::Entity::find_by_id(*id)
            .filter(task_attachments::Column::DeletedAt.is_null())
            .one(conn)
            .await?
            .ok_or_else(|| AttachmentError::InvalidInput(format!("附件不存在：{id}")))?;
        if attachment.task_id != task_id {
            return Err(AttachmentError::InvalidInput(
                "附件不属于当前任务，无法关联到评论".to_owned(),
            ));
        }
        if attachment.uploader_id != actor_user_id {
            return Err(AttachmentError::InvalidInput(
                "只能关联自己上传的附件".to_owned(),
            ));
        }
        if attachment.comment_id.is_some() {
            return Err(AttachmentError::InvalidInput(format!(
                "附件已被其他评论使用：{id}"
            )));
        }
        let mut active: task_attachments::ActiveModel = attachment.into();
        active.comment_id = Set(Some(comment_id));
        active.update(conn).await?;
    }
    Ok(())
}

/// 批量取多个评论的附件视图(评论列表/创建响应共用)。
pub async fn views_for_comments(
    db: &DatabaseConnection,
    comment_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<AttachmentView>>, AttachmentError> {
    let mut result: HashMap<Uuid, Vec<AttachmentView>> = HashMap::new();
    if comment_ids.is_empty() {
        return Ok(result);
    }
    let rows = task_attachments::Entity::find()
        .filter(task_attachments::Column::CommentId.is_in(comment_ids.to_vec()))
        .filter(task_attachments::Column::DeletedAt.is_null())
        .order_by_asc(task_attachments::Column::CreatedAt)
        .find_also_related(users::Entity)
        .all(db)
        .await?;
    for (attachment, uploader) in rows {
        let comment_id = attachment.comment_id;
        let view = view_from(
            attachment,
            uploader.map(|user| user.display_name).unwrap_or_default(),
        );
        if let Some(comment_id) = comment_id {
            result.entry(comment_id).or_default().push(view);
        }
    }
    Ok(result)
}

