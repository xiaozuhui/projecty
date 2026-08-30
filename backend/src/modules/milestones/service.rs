//! 项目状态排序和里程碑的应用服务。
use chrono::{DateTime, NaiveDate, Utc};
use projecty_entity::{milestones, operation_logs, project_members, project_statuses, projects};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::{http::extractors::CurrentUser, modules::tasks::service::user_can_read_project};

#[derive(Debug, thiserror::Error)]
pub enum MilestoneError {
    #[error("项目或里程碑不存在")]
    NotFound,
    #[error("没有项目管理权限")]
    Forbidden,
    #[error("请求参数无效：{0}")]
    InvalidInput(String),
    #[error("数据库错误：{0}")]
    Database(#[from] sea_orm::DbErr),
    #[error("数据序列化错误：{0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Serialize)]
pub struct ProjectStatusView {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub category: String,
    pub sort_order: i32,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, Serialize)]
pub struct MilestoneView {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub due_date: Option<NaiveDate>,
    pub is_reached: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}
#[derive(Debug, Serialize)]
pub struct MilestoneListResponse {
    pub items: Vec<MilestoneView>,
}
#[derive(Debug, Deserialize)]
pub struct StatusOrderRequest {
    pub status_ids: Vec<Uuid>,
}
#[derive(Debug, Deserialize)]
pub struct CreateMilestoneRequest {
    pub name: String,
    pub due_date: Option<NaiveDate>,
}
#[derive(Debug, Deserialize)]
pub struct UpdateMilestoneRequest {
    pub name: Option<String>,
    pub due_date: Option<NaiveDate>,
    pub is_reached: Option<bool>,
}
#[derive(Debug, Deserialize)]
pub struct DeleteMilestoneRequest {
    pub reason: Option<String>,
}

pub async fn list_statuses(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_key: &str,
) -> Result<Vec<ProjectStatusView>, MilestoneError> {
    let project = find_project(db, project_key).await?;
    if !user_can_read_project(db, current_user, project.id).await? {
        return Err(MilestoneError::Forbidden);
    }
    Ok(project_statuses::Entity::find()
        .filter(project_statuses::Column::ProjectId.eq(project.id))
        .order_by_asc(project_statuses::Column::SortOrder)
        .order_by_asc(project_statuses::Column::Id)
        .all(db)
        .await?
        .into_iter()
        .map(Into::into)
        .collect())
}

pub async fn reorder_statuses(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_key: &str,
    request: StatusOrderRequest,
) -> Result<Vec<ProjectStatusView>, MilestoneError> {
    let project = find_project(db, project_key).await?;
    require_manager(db, current_user, project.id).await?;
    let statuses = project_statuses::Entity::find()
        .filter(project_statuses::Column::ProjectId.eq(project.id))
        .all(db)
        .await?;
    if statuses.len() != request.status_ids.len()
        || statuses.iter().any(|s| !request.status_ids.contains(&s.id))
    {
        return Err(MilestoneError::InvalidInput(
            "状态排序必须包含当前项目的全部状态，且不能重复".to_owned(),
        ));
    }
    let txn = db.begin().await?;
    for (index, id) in request.status_ids.iter().enumerate() {
        let model = statuses
            .iter()
            .find(|s| s.id == *id)
            .ok_or_else(|| MilestoneError::InvalidInput("状态不存在".to_owned()))?;
        let mut active: project_statuses::ActiveModel = model.clone().into();
        active.sort_order = Set(index as i32);
        active.update(&txn).await?;
    }
    write_log(
        &txn,
        current_user.user_id,
        Some(project.id),
        None,
        "status_reordered",
        "重新排列项目任务状态".to_owned(),
        json!({"status_ids": request.status_ids}),
    )
    .await?;
    txn.commit().await?;
    list_statuses(db, current_user, project_key).await
}

pub async fn list(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_key: &str,
) -> Result<MilestoneListResponse, MilestoneError> {
    let project = find_project(db, project_key).await?;
    if !user_can_read_project(db, current_user, project.id).await? {
        return Err(MilestoneError::Forbidden);
    }
    let items = milestones::Entity::find()
        .filter(milestones::Column::ProjectId.eq(project.id))
        .filter(milestones::Column::DeletedAt.is_null())
        .order_by_asc(milestones::Column::DueDate)
        .order_by_desc(milestones::Column::CreatedAt)
        .all(db)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();
    Ok(MilestoneListResponse { items })
}

pub async fn create(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_key: &str,
    request: CreateMilestoneRequest,
) -> Result<MilestoneView, MilestoneError> {
    let project = find_project(db, project_key).await?;
    require_manager(db, current_user, project.id).await?;
    let name = required_name(request.name)?;
    let now = Utc::now();
    let txn = db.begin().await?;
    let model = milestones::ActiveModel {
        id: Set(Uuid::now_v7()),
        project_id: Set(project.id),
        name: Set(name.clone()),
        due_date: Set(request.due_date),
        is_reached: Set(false),
        created_at: Set(now),
        updated_at: Set(now),
        deleted_at: Set(None),
        deleted_by: Set(None),
        delete_reason: Set(None),
    }
    .insert(&txn)
    .await?;
    write_log(
        &txn,
        current_user.user_id,
        Some(project.id),
        None,
        "milestone_created",
        format!("创建里程碑：{name}"),
        json!({"name": name, "due_date": request.due_date}),
    )
    .await?;
    txn.commit().await?;
    Ok(model.into())
}

pub async fn update(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    id: Uuid,
    request: UpdateMilestoneRequest,
) -> Result<MilestoneView, MilestoneError> {
    let model = milestones::Entity::find_by_id(id)
        .filter(milestones::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(MilestoneError::NotFound)?;
    require_manager(db, current_user, model.project_id).await?;
    let mut active: milestones::ActiveModel = model.clone().into();
    let mut changes = serde_json::Map::new();
    if let Some(name) = request.name {
        let value = required_name(name)?;
        active.name = Set(value.clone());
        changes.insert("name".to_owned(), json!(value));
    }
    if let Some(due_date) = request.due_date {
        active.due_date = Set(Some(due_date));
        changes.insert("due_date".to_owned(), json!(due_date));
    }
    if let Some(is_reached) = request.is_reached {
        active.is_reached = Set(is_reached);
        changes.insert("is_reached".to_owned(), json!(is_reached));
    }
    if changes.is_empty() {
        return Ok(model.into());
    }
    active.updated_at = Set(Utc::now());
    let txn = db.begin().await?;
    let updated = active.update(&txn).await?;
    write_log(
        &txn,
        current_user.user_id,
        Some(model.project_id),
        None,
        "milestone_updated",
        format!("更新里程碑：{}", model.name),
        json!(changes),
    )
    .await?;
    txn.commit().await?;
    Ok(updated.into())
}

pub async fn delete(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    id: Uuid,
    request: DeleteMilestoneRequest,
) -> Result<(), MilestoneError> {
    let model = milestones::Entity::find_by_id(id)
        .filter(milestones::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(MilestoneError::NotFound)?;
    require_manager(db, current_user, model.project_id).await?;
    let now = Utc::now();
    let mut active: milestones::ActiveModel = model.clone().into();
    active.deleted_at = Set(Some(now));
    active.deleted_by = Set(Some(current_user.user_id));
    active.delete_reason = Set(request.reason.clone());
    active.updated_at = Set(now);
    let txn = db.begin().await?;
    active.update(&txn).await?;
    write_log(
        &txn,
        current_user.user_id,
        Some(model.project_id),
        None,
        "milestone_deleted",
        format!("逻辑删除里程碑：{}", model.name),
        json!({"reason": request.reason, "deleted_at": now}),
    )
    .await?;
    txn.commit().await?;
    Ok(())
}

async fn find_project(
    db: &DatabaseConnection,
    key: &str,
) -> Result<projects::Model, MilestoneError> {
    projects::Entity::find()
        .filter(projects::Column::ProjectKey.eq(key.trim().to_ascii_uppercase()))
        .filter(projects::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(MilestoneError::NotFound)
}
async fn require_manager(
    db: &DatabaseConnection,
    user: &CurrentUser,
    project_id: Uuid,
) -> Result<(), MilestoneError> {
    if user.system_role.is_super_admin() {
        return Ok(());
    }
    let count = project_members::Entity::find()
        .filter(project_members::Column::ProjectId.eq(project_id))
        .filter(project_members::Column::UserId.eq(user.user_id))
        .filter(project_members::Column::Role.eq("manager"))
        .filter(project_members::Column::RevokedAt.is_null())
        .count(db)
        .await?;
    if count > 0 {
        Ok(())
    } else {
        Err(MilestoneError::Forbidden)
    }
}
async fn write_log<C: ConnectionTrait + Send + Sync>(
    conn: &C,
    actor: Uuid,
    project_id: Option<Uuid>,
    task_id: Option<Uuid>,
    action: &str,
    summary: String,
    diff: serde_json::Value,
) -> Result<(), MilestoneError> {
    operation_logs::ActiveModel {
        id: Set(Uuid::now_v7()),
        actor_user_id: Set(actor),
        module: Set("milestone".to_owned()),
        action: Set(action.to_owned()),
        project_id: Set(project_id),
        task_id: Set(task_id),
        target_type: Set("milestone".to_owned()),
        target_id: Set(None),
        summary: Set(summary),
        diff: Set(Some(diff)),
        snapshot: Set(None),
        created_at: Set(Utc::now()),
    }
    .insert(conn)
    .await?;
    Ok(())
}
fn required_name(value: String) -> Result<String, MilestoneError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(MilestoneError::InvalidInput(
            "里程碑名称不能为空".to_owned(),
        ));
    }
    if value.chars().count() > 160 {
        return Err(MilestoneError::InvalidInput(
            "里程碑名称不能超过 160 个字符".to_owned(),
        ));
    }
    Ok(value.to_owned())
}
impl From<project_statuses::Model> for ProjectStatusView {
    fn from(v: project_statuses::Model) -> Self {
        Self {
            id: v.id,
            project_id: v.project_id,
            name: v.name,
            category: v.category,
            sort_order: v.sort_order,
            is_default: v.is_default,
            created_at: v.created_at,
        }
    }
}
impl From<milestones::Model> for MilestoneView {
    fn from(v: milestones::Model) -> Self {
        Self {
            id: v.id,
            project_id: v.project_id,
            name: v.name,
            due_date: v.due_date,
            is_reached: v.is_reached,
            created_at: v.created_at,
            updated_at: v.updated_at,
            deleted_at: v.deleted_at,
        }
    }
}
