//! 认证相关接口。
//! 除 `POST /api/auth/login` 外，本文件所有接口都要求已登录。

use axum::extract::State;

use crate::auth::AuthUser;
use crate::error::AppError;
use crate::handler::ApiJson;
use crate::model::user::{ChangePasswordRequest, LoginRequest, LoginResult};
use crate::response::{ok_empty, ApiResponse};
use crate::service::{auth_service, user_service};
use crate::state::AppState;

/// GET /api/health（需登录）
/// 运维探针：只回版本与基础计数，不含账号明细。
pub async fn health(
    State(state): State<AppState>,
    _user: AuthUser,
) -> Result<ApiResponse<serde_json::Value>, AppError> {
    let users = crate::db::user_db::count(&state.pool).await?;
    let projects: i64 = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM projects")
        .fetch_one(&state.pool)
        .await
        .map(|(count,)| count)
        .unwrap_or(0);

    Ok(ApiResponse::ok(serde_json::json!({
        "name": "RedTeam Atlas",
        "version": env!("CARGO_PKG_VERSION"),
        "users": users,
        "projects": projects,
        "encrypted": true
    })))
}

/// POST /api/auth/login（唯一公开接口）
pub async fn login(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<LoginRequest>,
) -> Result<ApiResponse<LoginResult>, AppError> {
    let result = auth_service::login(&state.pool, &state.cfg, req).await?;
    Ok(ApiResponse::ok(result))
}

/// POST /api/auth/logout
/// JWT 无状态，服务端无需处理；保留接口便于前端统一调用。
pub async fn logout(_user: AuthUser) -> ApiResponse<()> {
    ok_empty()
}

/// GET /api/auth/me
pub async fn me(user: AuthUser) -> ApiResponse<serde_json::Value> {
    ApiResponse::ok(serde_json::json!({
        "id": user.id,
        "username": user.username,
        "displayName": user.display_name,
        "role": user.role,
    }))
}

/// POST /api/auth/password
pub async fn change_password(
    State(state): State<AppState>,
    user: AuthUser,
    ApiJson(req): ApiJson<ChangePasswordRequest>,
) -> Result<ApiResponse<LoginResult>, AppError> {
    let result = auth_service::change_password(&state.pool, &state.cfg, &user.id, req).await?;
    Ok(ApiResponse::ok(result))
}

/// GET /api/auth/users
/// 成员名录（授权选择器用，只暴露 id/用户名/显示名）。
pub async fn directory(
    State(state): State<AppState>,
) -> Result<ApiResponse<Vec<crate::model::user::DirectoryEntry>>, AppError> {
    let list = user_service::directory(&state.pool).await?;
    Ok(ApiResponse::ok(list))
}