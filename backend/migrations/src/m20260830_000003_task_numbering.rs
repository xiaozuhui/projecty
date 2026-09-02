use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Projects::Table)
                    .add_column(
                        ColumnDef::new(Projects::TaskNumberSeed)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Tasks::Table)
                    .add_column(
                        ColumnDef::new(Tasks::TaskNumber)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .get_connection()
            .execute_raw(Statement::from_string(
                DatabaseBackend::Postgres,
                "WITH ranked AS (SELECT id, ROW_NUMBER() OVER (PARTITION BY project_id ORDER BY created_at, id) AS number FROM tasks) UPDATE tasks SET task_number = ranked.number FROM ranked WHERE tasks.id = ranked.id",
            ))
            .await?;
        manager
            .get_connection()
            .execute_raw(Statement::from_string(
                DatabaseBackend::Postgres,
                "UPDATE projects SET task_number_seed = COALESCE((SELECT MAX(task_number) FROM tasks WHERE tasks.project_id = projects.id), 0)",
            ))
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_tasks_project_task_number")
                    .table(Tasks::Table)
                    .col(Tasks::ProjectId)
                    .col(Tasks::TaskNumber)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_tasks_project_created")
                    .table(Tasks::Table)
                    .col(Tasks::ProjectId)
                    .col(Tasks::CreatedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_tasks_project_created")
                    .table(Tasks::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("uq_tasks_project_task_number")
                    .table(Tasks::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Tasks::Table)
                    .drop_column(Tasks::TaskNumber)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Projects::Table)
                    .drop_column(Projects::TaskNumberSeed)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Projects {
    Table,
    TaskNumberSeed,
}

#[derive(DeriveIden)]
enum Tasks {
    Table,
    ProjectId,
    TaskNumber,
    CreatedAt,
}
