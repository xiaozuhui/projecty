// SeaORM migration registry. Each m202... file is one versioned schema change.
pub use sea_orm_migration::prelude::*;

mod m20260830_000001_initial_schema;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20260830_000001_initial_schema::Migration)]
    }
}
