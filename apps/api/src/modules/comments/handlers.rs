use crate::http::error::{placeholder, ApiResponse};
pub async fn list() -> ApiResponse {
    placeholder("comments", "list")
}
pub async fn create() -> ApiResponse {
    placeholder("comments", "create")
}
pub async fn delete() -> ApiResponse {
    placeholder("comments", "logical_delete")
}
