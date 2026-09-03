use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "notifications")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    /// 接收人。
    pub user_id: Uuid,
    /// assigned / review_requested / commented / status_changed。
    pub r#type: String,
    /// 触发人显示名,写入时快照。
    pub actor_name: String,
    /// 任务 Key 快照,用于前端深链。
    pub task_key: String,
    pub project_key: String,
    /// 完整文案快照,任务后续改名不影响历史展示。
    pub summary: String,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
