//! 项目状态配置和里程碑服务入口。

use chrono::{DateTime, Utc};
use projecty_entity::{project_statuses, projects};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde::Serialize;
use uuid::Uuid;

use crate::{http::extractors::CurrentUser, modules::tasks::service::user_can_read_project};

#[derive(Debug, thiserror::Error)]
pub enum StatusError {
    #[error("项目不存在")]
    NotFound,
    #[error("没有项目读取权限")]
    Forbidden,
    #[error("数据库错误：{0}")]
    Database(#[from] sea_orm::DbErr),
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

pub async fn list_statuses(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_key: &str,
) -> Result<Vec<ProjectStatusView>, StatusError> {
    let project = projects::Entity::find()
        .filter(projects::Column::ProjectKey.eq(project_key.trim().to_ascii_uppercase()))
        .filter(projects::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(StatusError::NotFound)?;

    if !user_can_read_project(db, current_user, project.id).await? {
        return Err(StatusError::Forbidden);
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

impl From<project_statuses::Model> for ProjectStatusView {
    fn from(value: project_statuses::Model) -> Self {
        Self {
            id: value.id,
            project_id: value.project_id,
            name: value.name,
            category: value.category,
            sort_order: value.sort_order,
            is_default: value.is_default,
            created_at: value.created_at,
        }
    }
}
