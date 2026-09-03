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
        .route("/tasks", get(handlers::list_cross_project_tasks))
        .route(
            "/tasks/{task_key}",
            get(handlers::detail).patch(handlers::update),
        )
        .route("/tasks/{task_key}/transition", post(handlers::transition))
        .route("/tasks/{task_key}/move", post(handlers::move_task))
        .route("/tasks/{task_key}/delete", post(handlers::delete))
        .route("/tasks/{task_key}/restore", post(handlers::restore))
        .route(
            "/tasks/{task_key}/subtasks",
            get(handlers::subtasks).post(handlers::create_subtask),
        )
        .route(
            "/projects/{project_key}/labels",
            get(handlers::list_project_labels),
        )
        .route("/tasks/{task_key}/labels", post(handlers::add_task_label))
        .route(
            "/tasks/{task_key}/labels/{label_id}/delete",
            post(handlers::remove_task_label),
        )
        .route(
            "/tasks/{task_key}/dependencies",
            get(handlers::list_dependencies).post(handlers::add_dependency),
        )
        .route(
            "/tasks/{task_key}/dependencies/{dependency_id}/delete",
            post(handlers::remove_dependency),
        )
        .route("/tasks/{task_key}/copy", post(handlers::copy_task))
        .route(
            "/projects/{project_key}/tasks/deleted",
            get(handlers::list_deleted_tasks),
        )
        .route(
            "/projects/{project_key}/task-dependencies",
            get(handlers::list_project_dependencies),
        )
        .route(
            "/projects/{project_key}/tasks/export",
            get(handlers::export_project_tasks),
        )
}
