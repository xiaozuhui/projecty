use crate::http::error::{placeholder, ApiResponse};
pub async fn statuses() -> ApiResponse {
    placeholder("statuses", "list")
}
pub async fn reorder_statuses() -> ApiResponse {
    placeholder("statuses", "reorder")
}
pub async fn list() -> ApiResponse {
    placeholder("milestones", "list")
}
pub async fn create() -> ApiResponse {
    placeholder("milestones", "create")
}
pub async fn update() -> ApiResponse {
    placeholder("milestones", "update")
}
pub async fn delete() -> ApiResponse {
    placeholder("milestones", "logical_delete")
}
