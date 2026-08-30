use super::handlers;
use crate::state::AppState;
use axum::{
    routing::{get, patch, post},
    Router,
};
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/departments", get(handlers::list).post(handlers::create))
        .route("/departments/{department_id}", patch(handlers::update))
        .route(
            "/departments/{department_id}/delete",
            post(handlers::delete),
        )
        .route(
            "/departments/{department_id}/projects",
            get(handlers::projects),
        )
}
