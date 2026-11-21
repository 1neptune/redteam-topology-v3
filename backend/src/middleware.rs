//! 接口访问与错误日志的中间件：包住 `/api` 与 `/mcp`，把每次调用落成一行文件日志。

use std::net::SocketAddr;

use axum::body::{to_bytes, Body};
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, Request, StatusCode};
use axum::middleware::Next;
use axum::response::Response;

use crate::service::run_log;
use crate::state::AppState;

/// 读取请求/响应体的上限。必须足够大：读不通就等于把响应体丢掉，
/// 前端会收到空 body 并报「格式错误」。日志只在写盘时截断，不影响转发。
const MAX_BODY: usize = 48 * 1024 * 1024;

/// 写进日志的字节数
const LOG_BYTES: usize = 4096;

/// 取前若干字节的可读文本
fn head_text(bytes: &[u8]) -> String {
    let end = bytes.len().min(LOG_BYTES);
    String::from_utf8_lossy(&bytes[..end]).to_string()
}

/// 取来源地址：优先 `X-Forwarded-For`，其次连接地址
fn client_ip(headers: &HeaderMap, addr: Option<SocketAddr>) -> String {
    if let Some(value) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = value.split(',').next() {
            let trimmed = first.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    if let Some(value) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    addr.map(|a| a.ip().to_string()).unwrap_or_else(|| "-".to_string())
}

/// 记录一次接口调用
pub async fn record(State(state): State<AppState>, req: Request<Body>, next: Next) -> Response {
    let started = std::time::Instant::now();
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let query = req.uri().query().map(|q| format!("?{q}")).unwrap_or_default();
    let ip = client_ip(req.headers(), req.extensions().get::<ConnectInfo<SocketAddr>>().map(|c| c.0));

    // 读出请求体再原样放回，handler 仍然拿得到
    let (parts, body) = req.into_parts();
    let bytes = to_bytes(body, MAX_BODY).await.unwrap_or_default();
    let request_text = head_text(&bytes);
    let rebuilt = Request::from_parts(parts, Body::from(bytes));

    let response = next.run(rebuilt).await;
    let status = response.status();

    // 响应体只在大致是 JSON 时截取，图片等二进制跳过
    let (parts, body) = response.into_parts();
    let bytes = to_bytes(body, MAX_BODY).await.unwrap_or_default();
    let content_type = parts
        .headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let response_text = if content_type.contains("json") || content_type.contains("text") {
        head_text(&bytes)
    } else {
        format!("<{} 字节 {}>", bytes.len(), content_type)
    };

    let full_path = format!("{path}{query}");
    run_log::access(&state.cfg, &ip, method.as_str(), &full_path, &request_text, status.as_u16(), &response_text);

    // 4xx 与 5xx 都进错误日志：业务拒绝与运行异常同样需要可追查
    if status.is_client_error() || status.is_server_error() {
        run_log::error(&state.cfg, &ip, method.as_str(), &full_path, status.as_u16(), &response_text);
    }
    if status == StatusCode::INTERNAL_SERVER_ERROR {
        tracing::error!(%method, %full_path, elapsed_ms = started.elapsed().as_millis() as u64, "接口返回 500");
    }

    // 体积可能因重新序列化而变，content-length 以实际字节为准
    let mut parts = parts;
    parts.headers.remove(axum::http::header::CONTENT_LENGTH);
    if let Ok(value) = axum::http::HeaderValue::from_str(&bytes.len().to_string()) {
        parts.headers.insert(axum::http::header::CONTENT_LENGTH, value);
    }
    Response::from_parts(parts, Body::from(bytes))
}