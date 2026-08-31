use super::handlers;
use crate::state::AppState;
use axum::{
    routing::{get, patch, post},
    Router,
};
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/users", get(handlers::list).post(handlers::create))
        .route("/users/{user_id}", patch(handlers::update))
        .route("/users/import-template", get(handlers::import_template))
        .route("/users/import", post(handlers::import))
}
