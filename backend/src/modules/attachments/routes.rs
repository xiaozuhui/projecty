use super::handlers;
use crate::state::AppState;
use axum::{
    routing::{get, post},
    Router,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/tasks/{task_key}/attachments",
            get(handlers::list).post(handlers::upload),
        )
        .route("/attachments/{object_key}/content", get(handlers::content))
        .route("/attachments/{id}/delete", post(handlers::delete))
}
