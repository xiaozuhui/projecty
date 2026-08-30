use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "operation_logs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub actor_user_id: Uuid,
    pub module: String,
    pub action: String,
    pub project_id: Option<Uuid>,
    pub task_id: Option<Uuid>,
    pub target_type: String,
    pub target_id: Option<Uuid>,
    pub summary: String,
    pub diff: Option<Json>,
    pub snapshot: Option<Json>,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
