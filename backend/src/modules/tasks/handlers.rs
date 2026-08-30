use crate::http::error::{placeholder, ApiResponse};
pub async fn list_project_tasks() -> ApiResponse {
    placeholder("tasks", "list_project_tasks")
}
pub async fn create_project_task() -> ApiResponse {
    placeholder("tasks", "create_project_task")
}
pub async fn detail() -> ApiResponse {
    placeholder("tasks", "detail")
}
pub async fn update() -> ApiResponse {
    placeholder("tasks", "update")
}
pub async fn transition() -> ApiResponse {
    placeholder("tasks", "transition")
}
pub async fn delete() -> ApiResponse {
    placeholder("tasks", "logical_delete")
}
pub async fn restore() -> ApiResponse {
    placeholder("tasks", "restore")
}
pub async fn subtasks() -> ApiResponse {
    placeholder("tasks", "subtasks")
}
pub async fn create_subtask() -> ApiResponse {
    placeholder("tasks", "create_subtask")
}
