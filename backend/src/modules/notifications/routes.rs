use super::handlers;
use crate::state::AppState;
use axum::{
    routing::{get, post},
    Router,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/notifications", get(handlers::list))
        .route("/notifications/unread-count", get(handlers::unread_count))
        .route("/notifications/read-all", post(handlers::mark_all_read))
        .route("/notifications/{id}/read", post(handlers::mark_read))
}
