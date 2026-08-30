use super::handlers;
use crate::state::AppState;
use axum::{routing::get, Router};
pub fn routes() -> Router<AppState> {
    Router::new().route("/search", get(handlers::search))
}
