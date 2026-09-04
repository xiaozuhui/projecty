use crate::{config, http::middleware::access_log, http::routes::api_router, state::AppState};
use axum::{Router, extract::DefaultBodyLimit, middleware::from_fn};
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
};

pub fn build_router(state: AppState) -> Router {
    let static_dir = state.config.static_dir.clone();
    api_router()
        // axum 默认 2MB 上限会挡住上传,放宽到附件总上限加少量表单开销
        //(分片上传另有限制:单个分片请求体由路由级 DefaultBodyLimit 约束在 chunk 大小)。
        .layer(DefaultBodyLimit::max(config::UPLOAD_MAX_BYTES + 65_536))
        .layer(CorsLayer::permissive())
        // 最外层访问日志:2xx 一行 INFO,4xx/5xx 带响应体与耗时,替代默认等级下不输出的 TraceLayer。
        .layer(from_fn(access_log))
        .with_state(state)
        // 前端纯 SPA 静态托管:未命中 /healthz 与 /api/v1 的路径全部落到静态文件;
        // 目录内找不到的走 fallback 回 index.html 且带 200 状态码
        //(not_found_service 会保留 404,深链接刷新会拿到 404 页语义),
        // 由前端路由接管。静态路由挂在访问日志层之外,避免每个 _app 资产刷一行 INFO。
        .fallback_service(
            ServeDir::new(&static_dir).fallback(ServeFile::new(static_dir.join("index.html"))),
        )
}
