// SeaORM migration registry. Each m202... file is one versioned schema change.
pub use sea_orm_migration::prelude::*;

mod m20260830_000001_initial_schema;
mod m20260830_000002_auth_constraints;
mod m20260830_000003_task_numbering;
mod m20260830_000004_department_closure;
mod m20260831_000005_user_last_login_at;
mod m20260831_000006_task_attachments;
mod m20260831_000007_task_position;
mod m20260901_000008_task_reviewer;
mod m20260902_000009_user_email;
mod m20260903_000010_task_start_at;
mod m20260903_000011_task_type;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260830_000001_initial_schema::Migration),
            Box::new(m20260830_000002_auth_constraints::Migration),
            Box::new(m20260830_000003_task_numbering::Migration),
            Box::new(m20260830_000004_department_closure::Migration),
            Box::new(m20260831_000005_user_last_login_at::Migration),
            Box::new(m20260831_000006_task_attachments::Migration),
            Box::new(m20260831_000007_task_position::Migration),
            Box::new(m20260901_000008_task_reviewer::Migration),
            Box::new(m20260902_000009_user_email::Migration),
            Box::new(m20260903_000010_task_start_at::Migration),
            Box::new(m20260903_000011_task_type::Migration),
        ]
    }
}
