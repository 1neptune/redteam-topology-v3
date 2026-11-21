//! 系统信息接口：版本查询与运行日志。

use axum::extract::{Query, State};

use crate::auth::AuthUser;
use crate::error::AppError;
use crate::model::system::VersionInfo;
use crate::response::ApiResponse;
use crate::service::system_service;
use crate::state::AppState;

/// GET /api/system/version（需登录）
pub async fn version(
    State(state): State<AppState>,
    _user: AuthUser,
) -> Result<ApiResponse<VersionInfo>, AppError> {
    let info = system_service::version_info(&state.cfg)?;
    Ok(ApiResponse::ok(info))
}

/// GET /api/system/logs?kind=access|error&limit=200（仅管理员）
pub async fn logs(
    State(state): State<AppState>,
    user: AuthUser,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Result<ApiResponse<serde_json::Value>, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("只有管理员可以查看运行日志".to_string()));
    }
    let kind = query.get("kind").cloned().unwrap_or_else(|| "access".to_string());
    let limit = query
        .get("limit")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(200)
        .clamp(1, 1000);
    let (file, columns): (&str, &[&str]) = match kind.as_str() {
        "error" => ("error.log", &["at", "level", "ip", "endpoint", "status", "message"]),
        _ => ("access.log", &["at", "ip", "endpoint", "request", "status", "response"]),
    };
    let rows = crate::service::run_log::tail(&state.cfg, file, limit);
    Ok(ApiResponse::ok(serde_json::json!({
        "kind": kind,
        "columns": columns,
        "rows": rows,
    })))
}