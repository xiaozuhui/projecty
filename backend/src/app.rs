use crate::{http::routes::api_router, state::AppState};
use axum::{extract::DefaultBodyLimit, Router};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

pub fn build_router(state: AppState) -> Router {
    api_router()
        // axum 默认 2MB 上限会挡住图片上传,放宽到附件上限加少量表单开销。
        .layer(DefaultBodyLimit::max(
            state.config.upload_max_bytes + 65_536,
        ))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
