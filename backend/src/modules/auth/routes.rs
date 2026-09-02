use super::handlers;
use crate::state::AppState;
use axum::{
    routing::{get, patch, post},
    Router,
};
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(handlers::login))
        .route("/auth/refresh", post(handlers::refresh))
        .route("/auth/logout", post(handlers::logout))
        .route("/me", get(handlers::me).patch(handlers::update_profile))
        .route("/me/password", patch(handlers::change_password))
}
