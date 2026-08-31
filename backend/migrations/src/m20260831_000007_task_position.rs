use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Tasks::Table)
                    .add_column(
                        ColumnDef::new(Tasks::Position)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;
        // 回填:同一状态列内按创建时间排序编号,首行为 0,保证存量数据可确定性重放。
        manager
            .get_connection()
            .execute_unprepared(
                "UPDATE tasks SET position = ranked.rn - 1 FROM (SELECT id, ROW_NUMBER() OVER (PARTITION BY status_id ORDER BY created_at, id) AS rn FROM tasks) AS ranked WHERE tasks.id = ranked.id",
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_tasks_status_position")
                    .table(Tasks::Table)
                    .col(Tasks::StatusId)
                    .col(Tasks::Position)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_tasks_status_position")
                    .table(Tasks::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Tasks::Table)
                    .drop_column(Tasks::Position)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Tasks {
    Table,
    StatusId,
    Position,
}
