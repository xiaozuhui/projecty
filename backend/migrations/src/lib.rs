// SeaORM migration registry. Each m202... file is one versioned schema change.
pub use sea_orm_migration::prelude::*;

mod m20260830_000001_initial_schema;
mod m20260830_000002_auth_constraints;
mod m20260830_000003_task_numbering;
mod m20260830_000004_department_closure;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260830_000001_initial_schema::Migration),
            Box::new(m20260830_000002_auth_constraints::Migration),
            Box::new(m20260830_000003_task_numbering::Migration),
            Box::new(m20260830_000004_department_closure::Migration),
        ]
    }
}
