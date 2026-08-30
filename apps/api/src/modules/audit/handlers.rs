use crate::http::error::{placeholder, ApiResponse};
pub async fn project_logs() -> ApiResponse {
    placeholder("audit", "project_logs")
}
pub async fn export_project_logs() -> ApiResponse {
    placeholder("audit", "export_project_logs_unmasked")
}
pub async fn task_logs() -> ApiResponse {
    placeholder("audit", "task_logs")
}
pub async fn export_task_logs() -> ApiResponse {
    placeholder("audit", "export_task_logs_unmasked")
}
pub async fn export_admin_logs() -> ApiResponse {
    placeholder("audit", "export_admin_logs_unmasked")
}
