use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "task_attachments")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub task_id: Uuid,
    pub comment_id: Option<Uuid>,
    pub uploader_id: Uuid,
    pub file_name: String,
    pub object_key: String,
    pub mime_type: String,
    pub byte_size: i64,
    pub sha256: Option<String>,
    pub created_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::tasks::Entity",
        from = "Column::TaskId",
        to = "super::tasks::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Task,
    #[sea_orm(
        belongs_to = "super::task_comments::Entity",
        from = "Column::CommentId",
        to = "super::task_comments::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Comment,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UploaderId",
        to = "super::users::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Uploader,
}
impl Related<super::tasks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Task.def()
    }
}
impl Related<super::task_comments::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Comment.def()
    }
}
impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Uploader.def()
    }
}
impl ActiveModelBehavior for ActiveModel {}
