//! HTTP 层基础中间件说明。
//!
//! 当前 JWT 身份解析由 `CurrentUser` extractor 完成，
//! `TraceLayer` 和 CORS 在应用路由中统一配置。
