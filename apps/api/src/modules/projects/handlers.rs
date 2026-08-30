use crate::http::error::{placeholder, ApiResponse};
pub async fn list() -> ApiResponse {
    placeholder("projects", "list")
}
pub async fn create() -> ApiResponse {
    placeholder("projects", "create")
}
pub async fn detail() -> ApiResponse {
    placeholder("projects", "detail")
}
pub async fn update() -> ApiResponse {
    placeholder("projects", "update")
}
pub async fn archive() -> ApiResponse {
    placeholder("projects", "archive")
}
pub async fn restore() -> ApiResponse {
    placeholder("projects", "restore")
}
pub async fn delete() -> ApiResponse {
    placeholder("projects", "logical_delete")
}
pub async fn list_members() -> ApiResponse {
    placeholder("project_members", "list")
}
pub async fn add_member() -> ApiResponse {
    placeholder("project_members", "add")
}
pub async fn update_member() -> ApiResponse {
    placeholder("project_members", "update")
}
pub async fn revoke_member() -> ApiResponse {
    placeholder("project_members", "revoke")
}
pub async fn list_department_grants() -> ApiResponse {
    placeholder("project_department_grants", "list")
}
pub async fn grant_department() -> ApiResponse {
    placeholder("project_department_grants", "grant")
}
pub async fn revoke_department_grant() -> ApiResponse {
    placeholder("project_department_grants", "revoke")
}
