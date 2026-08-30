use crate::config::Config;
use sea_orm::DatabaseConnection;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: DatabaseConnection,
}
impl AppState {
    pub fn new(config: Config, db: DatabaseConnection) -> Self {
        Self { config, db }
    }
}
