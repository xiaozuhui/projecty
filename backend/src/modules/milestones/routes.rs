use super::handlers;
use crate::state::AppState;
use axum::{
    routing::{get, patch, post},
    Router,
};
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/projects/{project_key}/statuses", get(handlers::statuses))
        .route(
            "/projects/{project_key}/statuses/order",
            patch(handlers::reorder_statuses),
        )
        .route(
            "/projects/{project_key}/milestones",
            get(handlers::list).post(handlers::create),
        )
        .route("/milestones/{id}", patch(handlers::update))
        .route("/milestones/{id}/delete", post(handlers::delete))
}
