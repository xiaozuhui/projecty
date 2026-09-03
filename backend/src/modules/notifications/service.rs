//! 站内通知：触发动作在各自 service 的事务内同步写入，前端轮询拉取。
//! 不依赖任何中间件；文案写入时快照，任务后续改名不影响历史展示。
use chrono::{DateTime, Utc};
use projecty_entity::{notifications, tasks};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::http::extractors::CurrentUser;

#[derive(Debug, thiserror::Error)]
pub enum NotificationError {
    #[error("通知不存在")]
    NotFound,
    #[error("请求参数无效：{0}")]
    InvalidInput(String),
    #[error("数据库错误：{0}")]
    Database(#[from] sea_orm::DbErr),
}

/// 通知类型常量：分配、指定评审、评论、状态流转。
pub const KIND_ASSIGNED: &str = "assigned";
pub const KIND_REVIEW_REQUESTED: &str = "review_requested";
pub const KIND_COMMENTED: &str = "commented";
pub const KIND_STATUS_CHANGED: &str = "status_changed";

#[derive(Debug, Serialize)]
pub struct NotificationView {
    pub id: Uuid,
    pub r#type: String,
    pub summary: String,
    pub task_key: String,
    pub project_key: String,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct NotificationListResponse {
    pub items: Vec<NotificationView>,
    pub page: u64,
    pub page_size: u64,
    pub total: u64,
    pub unread_count: u64,
}

#[derive(Debug, Deserialize)]
pub struct ListNotificationsQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub unread_only: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct UnreadCountResponse {
    pub count: u64,
}

/// 在触发动作的同一事务内写入通知：接收人去重、排除操作者本人。
/// task 与 project 的 key/名称由调用方在事务外取好传入，保证快照一致。
pub async fn notify<C>(
    conn: &C,
    recipient_ids: &[Uuid],
    actor: &CurrentUser,
    actor_display_name: &str,
    task: &tasks::Model,
    project_key: &str,
    kind: &str,
    summary: String,
) -> Result<(), sea_orm::DbErr>
where
    C: ConnectionTrait + Send + Sync,
{
    let mut seen = std::collections::HashSet::new();
    let now = Utc::now();
    for user_id in recipient_ids {
        // 不通知操作者自己,同一次动作同一接收人只发一条。
        if *user_id == actor.user_id || !seen.insert(*user_id) {
            continue;
        }
        notifications::ActiveModel {
            id: Set(Uuid::now_v7()),
            user_id: Set(*user_id),
            r#type: Set(kind.to_owned()),
            actor_name: Set(actor_display_name.to_owned()),
            task_key: Set(task.task_key.clone()),
            project_key: Set(project_key.to_owned()),
            summary: Set(summary.clone()),
            read_at: Set(None),
            created_at: Set(now),
        }
        .insert(conn)
        .await?;
    }
    Ok(())
}

pub async fn list(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    query: &ListNotificationsQuery,
) -> Result<NotificationListResponse, NotificationError> {
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(30).clamp(1, 100);
    let unread_only = query.unread_only.unwrap_or(false);
    let base = || {
        let mut statement = notifications::Entity::find()
            .filter(notifications::Column::UserId.eq(current_user.user_id));
        if unread_only {
            statement = statement.filter(notifications::Column::ReadAt.is_null());
        }
        statement
    };
    let total = base().count(db).await?;
    let items = base()
        .order_by_desc(notifications::Column::CreatedAt)
        .order_by_desc(notifications::Column::Id)
        .offset((page - 1) * page_size)
        .limit(page_size)
        .all(db)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();
    let unread_count = unread_count(db, current_user).await?;
    Ok(NotificationListResponse {
        items,
        page,
        page_size,
        total,
        unread_count,
    })
}

pub async fn unread_count(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
) -> Result<u64, NotificationError> {
    Ok(notifications::Entity::find()
        .filter(notifications::Column::UserId.eq(current_user.user_id))
        .filter(notifications::Column::ReadAt.is_null())
        .count(db)
        .await?)
}

pub async fn mark_read(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    id: Uuid,
) -> Result<(), NotificationError> {
    let model = notifications::Entity::find_by_id(id)
        .filter(notifications::Column::UserId.eq(current_user.user_id))
        .one(db)
        .await?
        .ok_or(NotificationError::NotFound)?;
    if model.read_at.is_some() {
        return Ok(());
    }
    let mut active: notifications::ActiveModel = model.into();
    active.read_at = Set(Some(Utc::now()));
    active.update(db).await?;
    Ok(())
}

pub async fn mark_all_read(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
) -> Result<u64, NotificationError> {
    let unread = notifications::Entity::find()
        .filter(notifications::Column::UserId.eq(current_user.user_id))
        .filter(notifications::Column::ReadAt.is_null())
        .all(db)
        .await?;
    let now = Utc::now();
    let mut updated = 0;
    for model in unread {
        let mut active: notifications::ActiveModel = model.into();
        active.read_at = Set(Some(now));
        active.update(db).await?;
        updated += 1;
    }
    Ok(updated)
}

impl From<notifications::Model> for NotificationView {
    fn from(value: notifications::Model) -> Self {
        Self {
            id: value.id,
            r#type: value.r#type,
            summary: value.summary,
            task_key: value.task_key,
            project_key: value.project_key,
            read_at: value.read_at,
            created_at: value.created_at,
        }
    }
}

/// 供通知触达范围计算使用：任务的负责人、创建人、评审人集合（去重由 notify 内部完成）。
pub fn task_audience(task: &tasks::Model) -> Vec<Uuid> {
    let mut ids = vec![task.reporter_id];
    ids.extend(task.assignee_id);
    ids.extend(task.reviewer_id);
    ids
}
