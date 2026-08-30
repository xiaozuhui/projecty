use super::handlers;
use crate::state::AppState;
use axum::{
    routing::{get, post},
    Router,
};
pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/tasks/{task_key}/comments",
            get(handlers::list).post(handlers::create),
        )
        .route("/comments/{id}/delete", post(handlers::delete))
}
