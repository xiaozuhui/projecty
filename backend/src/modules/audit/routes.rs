use super::handlers;
use crate::state::AppState;
use axum::{routing::get, Router};
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/projects/{project_key}/logs", get(handlers::project_logs))
        .route(
            "/projects/{project_key}/logs/export",
            get(handlers::export_project_logs),
        )
        .route("/tasks/{task_key}/logs", get(handlers::task_logs))
        .route(
            "/tasks/{task_key}/logs/export",
            get(handlers::export_task_logs),
        )
        .route(
            "/admin/operation-logs/export",
            get(handlers::export_admin_logs),
        )
}
