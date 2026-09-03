//! 任务评论查询、创建和逻辑删除服务。
use chrono::{DateTime, Utc};
use projecty_entity::{operation_logs, projects, task_comments, tasks, users};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::{
    http::extractors::CurrentUser,
    modules::attachments::service::{self as attachment_service, AttachmentError, AttachmentView},
    modules::notifications::service as notification_service,
    modules::tasks::service::user_can_read_project,
};

#[derive(Debug, thiserror::Error)]
pub enum CommentError {
    #[error("任务或评论不存在")]
    NotFound,
    #[error("没有当前任务的操作权限")]
    Forbidden,
    #[error("请求参数无效：{0}")]
    InvalidInput(String),
    #[error("数据库错误：{0}")]
    Database(#[from] sea_orm::DbErr),
}

fn attachment_error(error: AttachmentError) -> CommentError {
    match error {
        AttachmentError::Database(db_error) => CommentError::Database(db_error),
        other => CommentError::InvalidInput(other.to_string()),
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateCommentRequest {
    pub body: String,
    pub attachment_ids: Option<Vec<Uuid>>,
}
#[derive(Debug, Deserialize)]
pub struct DeleteCommentRequest {
    pub reason: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct CommentView {
    pub id: Uuid,
    pub task_id: Uuid,
    pub author_id: Uuid,
    pub author_name: String,
    pub body: String,
    pub attachments: Vec<AttachmentView>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

pub async fn list(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    task_key: &str,
) -> Result<Vec<CommentView>, CommentError> {
    let task = find_task(db, task_key).await?;
    require_read(db, current_user, task.project_id).await?;
    let rows = task_comments::Entity::find()
        .filter(task_comments::Column::TaskId.eq(task.id))
        .filter(task_comments::Column::DeletedAt.is_null())
        .order_by_asc(task_comments::Column::CreatedAt)
        .find_also_related(users::Entity)
        .all(db)
        .await?;
    let mut views: Vec<CommentView> = rows
        .into_iter()
        .filter_map(|(comment, author)| {
            author.map(|u| CommentView {
                id: comment.id,
                task_id: comment.task_id,
                author_id: comment.author_id,
                author_name: u.display_name,
                body: comment.body,
                attachments: Vec::new(),
                created_at: comment.created_at,
                updated_at: comment.updated_at,
                deleted_at: comment.deleted_at,
            })
        })
        .collect();
    let comment_ids: Vec<Uuid> = views.iter().map(|view| view.id).collect();
    let by_comment = attachment_service::views_for_comments(db, &comment_ids)
        .await
        .map_err(attachment_error)?;
    for view in &mut views {
        if let Some(attachments) = by_comment.get(&view.id) {
            view.attachments = attachments.clone();
        }
    }
    Ok(views)
}

pub async fn create(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    task_key: &str,
    request: CreateCommentRequest,
) -> Result<CommentView, CommentError> {
    let task = find_task(db, task_key).await?;
    require_read(db, current_user, task.project_id).await?;
    let body = required_body(request.body)?;
    let author = users::Entity::find_by_id(current_user.user_id)
        .one(db)
        .await?
        .ok_or(CommentError::NotFound)?;
    let project = projects::Entity::find_by_id(task.project_id)
        .one(db)
        .await?
        .ok_or(CommentError::NotFound)?;
    let now = Utc::now();
    let txn = db.begin().await?;
    let comment = task_comments::ActiveModel {
        id: Set(Uuid::now_v7()),
        task_id: Set(task.id),
        author_id: Set(current_user.user_id),
        body: Set(body.clone()),
        created_at: Set(now),
        updated_at: Set(now),
        deleted_at: Set(None),
        deleted_by: Set(None),
        delete_reason: Set(None),
    }
    .insert(&txn)
    .await?;
    // 评论通知负责人、创建人、评审人(通知服务内部去重并排除评论作者)。
    notification_service::notify(
        &txn,
        &notification_service::task_audience(&task),
        current_user,
        &author.display_name,
        &task,
        &project.project_key,
        notification_service::KIND_COMMENTED,
        format!(
            "{} 评论了 {}「{}」",
            author.display_name, task.task_key, task.title
        ),
    )
    .await?;
    operation_logs::ActiveModel {
        id: Set(Uuid::now_v7()),
        actor_user_id: Set(current_user.user_id),
        module: Set("comment".to_owned()),
        action: Set("comment_created".to_owned()),
        project_id: Set(Some(task.project_id)),
        task_id: Set(Some(task.id)),
        target_type: Set("comment".to_owned()),
        target_id: Set(Some(comment.id)),
        summary: Set(format!("添加任务评论：{}", task.task_key)),
        diff: Set(Some(json!({"body": body}))),
        snapshot: Set(None),
        created_at: Set(now),
    }
    .insert(&txn)
    .await?;
    let attachment_ids = request.attachment_ids.unwrap_or_default();
    if !attachment_ids.is_empty() {
        attachment_service::link_to_comment(
            &txn,
            current_user.user_id,
            task.id,
            comment.id,
            &attachment_ids,
        )
        .await
        .map_err(attachment_error)?;
    }
    txn.commit().await?;
    let by_comment = attachment_service::views_for_comments(db, &[comment.id])
        .await
        .map_err(attachment_error)?;
    Ok(CommentView {
        id: comment.id,
        task_id: comment.task_id,
        author_id: comment.author_id,
        author_name: author.display_name,
        body: comment.body,
        attachments: by_comment.get(&comment.id).cloned().unwrap_or_default(),
        created_at: comment.created_at,
        updated_at: comment.updated_at,
        deleted_at: comment.deleted_at,
    })
}

pub async fn delete(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    id: Uuid,
    request: DeleteCommentRequest,
) -> Result<(), CommentError> {
    let comment = task_comments::Entity::find_by_id(id)
        .filter(task_comments::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(CommentError::NotFound)?;
    let task = tasks::Entity::find_by_id(comment.task_id)
        .one(db)
        .await?
        .ok_or(CommentError::NotFound)?;
    require_read(db, current_user, task.project_id).await?;
    if comment.author_id != current_user.user_id && !current_user.system_role.is_super_admin() {
        return Err(CommentError::Forbidden);
    }
    let now = Utc::now();
    let mut active: task_comments::ActiveModel = comment.clone().into();
    active.deleted_at = Set(Some(now));
    active.deleted_by = Set(Some(current_user.user_id));
    active.delete_reason = Set(request.reason.clone());
    active.updated_at = Set(now);
    let txn = db.begin().await?;
    active.update(&txn).await?;
    operation_logs::ActiveModel {
        id: Set(Uuid::now_v7()),
        actor_user_id: Set(current_user.user_id),
        module: Set("comment".to_owned()),
        action: Set("comment_deleted".to_owned()),
        project_id: Set(Some(task.project_id)),
        task_id: Set(Some(task.id)),
        target_type: Set("comment".to_owned()),
        target_id: Set(Some(comment.id)),
        summary: Set(format!("逻辑删除任务评论：{}", task.task_key)),
        diff: Set(Some(json!({"reason": request.reason, "deleted_at": now}))),
        snapshot: Set(None),
        created_at: Set(now),
    }
    .insert(&txn)
    .await?;
    txn.commit().await?;
    Ok(())
}

async fn find_task(db: &DatabaseConnection, key: &str) -> Result<tasks::Model, CommentError> {
    tasks::Entity::find()
        .filter(tasks::Column::TaskKey.eq(key.trim().to_ascii_uppercase()))
        .filter(tasks::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(CommentError::NotFound)
}
async fn require_read(
    db: &DatabaseConnection,
    u: &CurrentUser,
    p: Uuid,
) -> Result<(), CommentError> {
    if user_can_read_project(db, u, p).await? {
        Ok(())
    } else {
        Err(CommentError::Forbidden)
    }
}
fn required_body(body: String) -> Result<String, CommentError> {
    let value = body.trim();
    if value.is_empty() {
        Err(CommentError::InvalidInput("评论内容不能为空".to_owned()))
    } else if value.chars().count() > 10_000 {
        Err(CommentError::InvalidInput(
            "评论内容不能超过 10000 个字符".to_owned(),
        ))
    } else {
        Ok(value.to_owned())
    }
}
