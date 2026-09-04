use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 附件分片上传会话:同一 uploader+task+client_file_key 的 active 会话可续传,
/// complete 后记录 attachment_id 保证幂等。
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "attachment_upload_sessions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub task_id: Uuid,
    pub uploader_id: Uuid,
    pub client_file_key: String,
    pub file_name: String,
    pub declared_mime: Option<String>,
    pub total_bytes: i64,
    pub chunk_size: i64,
    pub status: String,
    pub attachment_id: Option<Uuid>,
    pub client_sha256: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
