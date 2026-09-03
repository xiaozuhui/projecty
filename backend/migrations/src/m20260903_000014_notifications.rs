use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Notifications::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Notifications::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Notifications::UserId).uuid().not_null())
                    .col(
                        ColumnDef::new(Notifications::Type)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Notifications::ActorName)
                            .string_len(80)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Notifications::TaskKey)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Notifications::ProjectKey)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Notifications::Summary)
                            .string_len(300)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Notifications::ReadAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Notifications::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_notifications_user_created")
                    .table(Notifications::Table)
                    .col(Notifications::UserId)
                    .col(Notifications::CreatedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Notifications::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Notifications {
    Table,
    Id,
    UserId,
    Type,
    ActorName,
    TaskKey,
    ProjectKey,
    Summary,
    ReadAt,
    CreatedAt,
}
