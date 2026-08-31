use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TaskAttachments::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TaskAttachments::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(TaskAttachments::TaskId).uuid().not_null())
                    .col(ColumnDef::new(TaskAttachments::CommentId).uuid())
                    .col(
                        ColumnDef::new(TaskAttachments::UploaderId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaskAttachments::FileName)
                            .string_len(200)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaskAttachments::ObjectKey)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaskAttachments::MimeType)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaskAttachments::ByteSize)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaskAttachments::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(TaskAttachments::DeletedAt).timestamp_with_time_zone())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_task_attachments_object_key")
                    .table(TaskAttachments::Table)
                    .col(TaskAttachments::ObjectKey)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_task_attachments_task")
                    .table(TaskAttachments::Table)
                    .col(TaskAttachments::TaskId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_task_attachments_comment")
                    .table(TaskAttachments::Table)
                    .col(TaskAttachments::CommentId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TaskAttachments::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum TaskAttachments {
    Table,
    Id,
    TaskId,
    CommentId,
    UploaderId,
    FileName,
    ObjectKey,
    MimeType,
    ByteSize,
    CreatedAt,
    DeletedAt,
}
