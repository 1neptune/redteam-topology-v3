//! MCP（Model Context Protocol）服务端实现：JSON-RPC 2.0 over HTTP。
//! 鉴权支持两种方式：
//! - `Authorization: Bearer <token>`：网页/Agent 复用的 JWT
//! - `Authorization: Basic base64(用户名:口令)`：便于只支持 HTTP 基本认证的 Agent

pub mod export;
pub mod render;
pub mod richtext;
pub mod tools;

use axum::extract::{FromRequestParts, State};
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use axum::Json;
use base64::Engine;
use serde_json::{json, Value};

use crate::auth::AuthUser;
use crate::error::AppError;
use crate::service::auth_service;
use crate::state::AppState;

const PROTOCOL_VERSIONS: [&str; 3] = ["2024-11-05", "2025-03-26", "2025-06-18"];

/// MCP 调用者：从 Bearer 令牌或 Basic 凭据解析出的身份
pub struct McpUser(pub AuthUser);

/// 解析 HTTP Basic 凭据
fn basic_credentials(parts: &Parts) -> Option<(String, String)> {
    let raw = parts.headers.get(AUTHORIZATION)?.to_str().ok()?;
    let encoded = raw.strip_prefix("Basic ").or_else(|| raw.strip_prefix("basic "))?.trim();
    let bytes = base64::engine::general_purpose::STANDARD.decode(encoded).ok()?;
    let text = String::from_utf8(bytes).ok()?;
    let (username, password) = text.split_once(':')?;
    if username.is_empty() || password.is_empty() {
        return None;
    }
    Some((username.to_string(), password.to_string()))
}

impl FromRequestParts<AppState> for McpUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        if let Some((username, password)) = basic_credentials(parts) {
            let user = auth_service::verify_credentials(&state.pool, &username, &password).await?;
            return Ok(McpUser(user));
        }
        let user = AuthUser::from_request_parts(parts, state).await?;
        Ok(McpUser(user))
    }
}

/// JSON-RPC 错误对象
fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}

/// 包装工具成功返回
fn rpc_result(id: Value, payload: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [{ "type": "text", "text": payload.to_string() }],
            "isError": false
        }
    })
}

/// 工具执行失败时按 MCP 语义返回 isError
fn rpc_tool_error(id: Value, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [{ "type": "text", "text": message }],
            "isError": true
        }
    })
}

/// POST /mcp
pub async fn handle(
    State(state): State<AppState>,
    McpUser(user): McpUser,
    Json(body): Json<Value>,
) -> Response {
    let id = body.get("id").cloned().unwrap_or(Value::Null);
    let method = body.get("method").and_then(|m| m.as_str()).unwrap_or_default();

    // 通知类消息无需响应体
    if method.starts_with("notifications/") {
        return axum::http::StatusCode::ACCEPTED.into_response();
    }

    let response = match method {
        "initialize" => {
            let requested = body
                .pointer("/params/protocolVersion")
                .and_then(|v| v.as_str())
                .unwrap_or(PROTOCOL_VERSIONS[0]);
            let version = if PROTOCOL_VERSIONS.contains(&requested) {
                requested
            } else {
                PROTOCOL_VERSIONS[PROTOCOL_VERSIONS.len() - 1]
            };
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": { "name": "redteam-atlas", "version": env!("CARGO_PKG_VERSION") }
                }
            })
        }
        "ping" => json!({ "jsonrpc": "2.0", "id": id, "result": {} }),
        "tools/list" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": { "tools": tools::definitions() }
        }),
        "tools/call" => {
            let name = body
                .pointer("/params/name")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let args = body
                .pointer("/params/arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));

            if name.is_empty() {
                rpc_error(id, -32602, "缺少工具名称")
            } else {
                match tools::call(&state, &user, name, &args).await {
                    Ok(payload) => rpc_result(id, payload),
                    Err(AppError::BadRequest(msg))
                    | Err(AppError::Forbidden(msg))
                    | Err(AppError::NotFound(msg))
                    | Err(AppError::Conflict(msg)) => rpc_tool_error(id, &msg),
                    Err(err) => {
                        tracing::error!(error = %err, "MCP 工具执行失败");
                        rpc_tool_error(id, "工具执行失败")
                    }
                }
            }
        }
        "" => rpc_error(id, -32600, "无效请求"),
        other => rpc_error(id, -32601, &format!("未实现的方法：{other}")),
    };

    (axum::http::StatusCode::OK, Json(response)).into_response()
}