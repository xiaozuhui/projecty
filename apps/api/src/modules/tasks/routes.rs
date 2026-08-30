use super::handlers;
use crate::state::AppState;
use axum::{
    routing::{get, post},
    Router,
};
pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/projects/{project_key}/tasks",
            get(handlers::list_project_tasks).post(handlers::create_project_task),
        )
        .route(
            "/tasks/{task_key}",
            get(handlers::detail).patch(handlers::update),
        )
        .route("/tasks/{task_key}/transition", post(handlers::transition))
        .route("/tasks/{task_key}/delete", post(handlers::delete))
        .route("/tasks/{task_key}/restore", post(handlers::restore))
        .route(
            "/tasks/{task_key}/subtasks",
            get(handlers::subtasks).post(handlers::create_subtask),
        )
}
