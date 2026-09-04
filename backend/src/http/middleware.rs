//! HTTP 层基础中间件说明。
//!
//! 当前 JWT 身份解析由 `CurrentUser` extractor 完成，
//! CORS 在应用路由中统一配置。

use axum::{
    body::{Body, to_bytes},
    extract::Request,
    http::header::CONTENT_LENGTH,
    middleware::Next,
    response::Response,
};

/// 错误响应体最多读取的字节数:错误 envelope / rejection 文本都很小,
/// 超过说明不是可读文本,放弃记录(请求体本身不受影响,不会去碰附件下载流)。
const ERROR_BODY_LOG_LIMIT: usize = 4096;
/// 日志里展示的响应体长度上限,截断避免刷屏。
const ERROR_BODY_DISPLAY_CHARS: usize = 300;

/// 访问日志:每个请求一行 INFO(方法/路径/状态/耗时),
/// 4xx/5xx 额外带响应体与请求 Content-Length——参数反序列化失败、
/// 请求体超限这类被 axum 拒绝的请求在默认日志级别下就能看到原因。
pub async fn access_log(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let content_length = request
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let started = std::time::Instant::now();

    let response = next.run(request).await;
    let status = response.status();
    let elapsed_ms = started.elapsed().as_millis() as u64;

    if status.is_success() || status.is_redirection() {
        tracing::info!(%method, %uri, status = status.as_u16(), elapsed_ms, "request completed");
        return response;
    }

    // 读出错误响应体记日志后原样放回;失败的都是小 JSON/纯文本,
    // 成功响应(含附件 206 分段流)在上面提前返回,不会被缓冲。
    let (mut parts, body) = response.into_parts();
    let logged = to_bytes(body, ERROR_BODY_LOG_LIMIT).await.ok();
    if logged.is_none() {
        // 读不回来就不能保留旧 Content-Length,否则响应长度对不上会挂起客户端。
        parts.headers.remove(CONTENT_LENGTH);
    }
    let body_text = logged.as_ref().map(|bytes| {
        String::from_utf8_lossy(bytes)
            .chars()
            .take(ERROR_BODY_DISPLAY_CHARS)
            .collect::<String>()
    });
    let response = Response::from_parts(parts, Body::from(logged.unwrap_or_default()));

    if status.is_server_error() {
        tracing::error!(
            %method, %uri, status = status.as_u16(), elapsed_ms,
            body = body_text.as_deref().unwrap_or("<unreadable>"),
            "request failed"
        );
    } else {
        tracing::warn!(
            %method, %uri, status = status.as_u16(), elapsed_ms,
            content_length = content_length.as_deref().unwrap_or(""),
            body = body_text.as_deref().unwrap_or("<unreadable>"),
            "request rejected"
        );
    }
    response
}
