use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Labels::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Labels::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Labels::ProjectId).uuid().not_null())
                    .col(
                        ColumnDef::new(Labels::Name)
                            .string_len(40)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Labels::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_labels_project_name")
                    .table(Labels::Table)
                    .col(Labels::ProjectId)
                    .col(Labels::Name)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(TaskLabels::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(TaskLabels::TaskId).uuid().not_null())
                    .col(ColumnDef::new(TaskLabels::LabelId).uuid().not_null())
                    .col(
                        ColumnDef::new(TaskLabels::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(TaskLabels::TaskId)
                            .col(TaskLabels::LabelId),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_task_labels_label")
                    .table(TaskLabels::Table)
                    .col(TaskLabels::LabelId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TaskLabels::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Labels::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Labels {
    Table,
    Id,
    ProjectId,
    Name,
    CreatedAt,
}

#[derive(DeriveIden)]
enum TaskLabels {
    Table,
    TaskId,
    LabelId,
    CreatedAt,
}
