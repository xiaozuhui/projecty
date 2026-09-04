use crate::{config, http::middleware::access_log, http::routes::api_router, state::AppState};
use axum::{Router, extract::DefaultBodyLimit, middleware::from_fn};
use tower_http::cors::CorsLayer;

pub fn build_router(state: AppState) -> Router {
    api_router()
        // axum 默认 2MB 上限会挡住上传,放宽到附件总上限加少量表单开销
        //(分片上传另有限制:单个分片请求体由路由级 DefaultBodyLimit 约束在 chunk 大小)。
        .layer(DefaultBodyLimit::max(config::UPLOAD_MAX_BYTES + 65_536))
        .layer(CorsLayer::permissive())
        // 最外层访问日志:2xx 一行 INFO,4xx/5xx 带响应体与耗时,替代默认等级下不输出的 TraceLayer。
        .layer(from_fn(access_log))
        .with_state(state)
}
