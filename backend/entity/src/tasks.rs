use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "tasks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub task_key: String,
    pub project_id: Uuid,
    pub parent_task_id: Option<Uuid>,
    pub status_id: Uuid,
    pub milestone_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub priority: String,
    pub reporter_id: Uuid,
    pub assignee_id: Option<Uuid>,
    pub due_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by: Option<Uuid>,
    pub delete_reason: Option<String>,
    pub task_number: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
