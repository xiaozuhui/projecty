use crate::{http::routes::api_router, state::AppState};
use axum::Router;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

pub fn build_router(state: AppState) -> Router {
    api_router()
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
