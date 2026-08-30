use crate::http::error::{placeholder, ApiResponse};
pub async fn login() -> ApiResponse {
    placeholder("auth", "login")
}
pub async fn refresh() -> ApiResponse {
    placeholder("auth", "refresh")
}
pub async fn logout() -> ApiResponse {
    placeholder("auth", "logout")
}
pub async fn me() -> ApiResponse {
    placeholder("auth", "me")
}
pub async fn change_password() -> ApiResponse {
    placeholder("auth", "change_password")
}
