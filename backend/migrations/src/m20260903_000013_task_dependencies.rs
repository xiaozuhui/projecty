use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TaskDependencies::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TaskDependencies::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(TaskDependencies::TaskId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaskDependencies::DependsOnTaskId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaskDependencies::CreatedBy)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaskDependencies::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_task_dependencies_pair")
                    .table(TaskDependencies::Table)
                    .col(TaskDependencies::TaskId)
                    .col(TaskDependencies::DependsOnTaskId)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_task_dependencies_depends_on")
                    .table(TaskDependencies::Table)
                    .col(TaskDependencies::DependsOnTaskId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TaskDependencies::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum TaskDependencies {
    Table,
    Id,
    TaskId,
    DependsOnTaskId,
    CreatedBy,
    CreatedAt,
}
