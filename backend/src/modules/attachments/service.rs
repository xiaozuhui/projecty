//! 任务附件:图片写入本地磁盘、按不可猜的 object_key 公开读取、软删除与审计。
use std::collections::HashMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use projecty_entity::{operation_logs, task_attachments, tasks, users};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, Set, TransactionTrait,
};
use serde::Serialize;
use uuid::Uuid;

use crate::{http::extractors::CurrentUser, modules::tasks::service::user_can_read_project};

#[derive(Debug, thiserror::Error)]
pub enum AttachmentError {
    #[error("附件或任务不存在")]
    NotFound,
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

/// 声明 content_type 规范化:截掉 ; 参数,空白视为未声明,兜底 octet-stream。
fn normalize_declared_mime(content_type: Option<&str>) -> String {
    content_type
        .map(|value| value.split(';').next().unwrap_or("").trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("application/octet-stream")
        .to_owned()
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

pub struct IncomingFile {
    pub name: Option<String>,
    pub content_type: Option<String>,
    pub bytes: Vec<u8>,
}

pub async fn upload(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    task_key: &str,
    file: IncomingFile,
    upload_dir: &Path,
    max_bytes: usize,
) -> Result<AttachmentView, AttachmentError> {
    let task = find_task(db, task_key).await?;
    if !user_can_read_project(db, current_user, task.project_id).await? {
        return Err(AttachmentError::Forbidden);
    }
    let bytes = file.bytes;
    if bytes.is_empty() {
        return Err(AttachmentError::InvalidInput("上传文件不能为空".to_owned()));
    }
    if bytes.len() > max_bytes {
        return Err(AttachmentError::InvalidInput(format!(
            "文件大小不能超过 {} MB",
            max_bytes / 1024 / 1024
        )));
    }
    // 图片按嗅探结果定型(防改扩展名伪装);其他文件信任声明的 content_type,
    // 扩展名从原始文件名提取并白名单化,object_key 字符集不受影响。
    let (mime, extension) = match sniff_image_mime(&bytes) {
        Some(sniffed) => {
            if let Some(declared) = file.content_type.as_deref() {
                let declared = declared.split(';').next().unwrap_or("").trim();
                if !declared.is_empty() && declared != sniffed {
                    return Err(AttachmentError::InvalidInput(format!(
                        "文件内容与声明类型不符：声明 {declared}，实际 {sniffed}"
                    )));
                }
            }
            (sniffed.to_owned(), extension_for(sniffed).to_owned())
        }
        None => (
            normalize_declared_mime(file.content_type.as_deref()),
            safe_extension(file.name.as_deref()),
        ),
    };
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
        byte_size: Set(bytes.len() as i64),
        created_at: Set(now),
        deleted_at: Set(None),
    };
    let txn = db.begin().await?;
    let inserted = attachment.insert(&txn).await?;
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
    .insert(&txn)
    .await?;
    // 事务内先落盘再提交:写盘失败时数据库行一并回滚,不留指向缺失文件的记录。
    tokio::fs::create_dir_all(upload_dir)
        .await
        .map_err(|error| AttachmentError::Io(error.to_string()))?;
    tokio::fs::write(upload_dir.join(&object_key), &bytes)
        .await
        .map_err(|error| AttachmentError::Io(error.to_string()))?;
    txn.commit().await?;
    let uploader = users::Entity::find_by_id(current_user.user_id)
        .one(db)
        .await?
        .ok_or(AttachmentError::NotFound)?;
    Ok(view_from(inserted, uploader.display_name))
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

/// 公开读取入口:仅凭库中存在且未软删的 object_key 命中,返回字节、MIME 与原始文件名。
pub async fn read_content(
    db: &DatabaseConnection,
    object_key: &str,
    upload_dir: &Path,
) -> Result<(Vec<u8>, String, String), AttachmentError> {
    if !valid_object_key(object_key) {
        return Err(AttachmentError::NotFound);
    }
    let attachment = task_attachments::Entity::find()
        .filter(task_attachments::Column::ObjectKey.eq(object_key))
        .filter(task_attachments::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(AttachmentError::NotFound)?;
    let bytes = tokio::fs::read(upload_dir.join(object_key))
        .await
        .map_err(|error| AttachmentError::Io(error.to_string()))?;
    Ok((bytes, attachment.mime_type, attachment.file_name))
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
