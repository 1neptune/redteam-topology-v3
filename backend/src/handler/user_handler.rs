//! 用户管理接口（管理员）。

use axum::extract::{Path, State};

use crate::auth::AuthUser;
use crate::error::AppError;
use crate::handler::ApiJson;
use crate::model::user::{CreateUserRequest, UpdateUserRequest};
use crate::model::PublicUser;
use crate::response::ApiResponse;
use crate::service::user_service;
use crate::state::AppState;

/// GET /api/users
pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<ApiResponse<Vec<PublicUser>>, AppError> {
    let list = user_service::list(&state.pool, &user).await?;
    Ok(ApiResponse::ok(list))
}

/// POST /api/users
pub async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    ApiJson(req): ApiJson<CreateUserRequest>,
) -> Result<ApiResponse<PublicUser>, AppError> {
    let created = user_service::create(&state.pool, &user, req).await?;
    Ok(ApiResponse::ok(created))
}

/// PATCH /api/users/{id}
pub async fn update(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    ApiJson(req): ApiJson<UpdateUserRequest>,
) -> Result<ApiResponse<PublicUser>, AppError> {
    let updated = user_service::update(&state.pool, &user, &id, req).await?;
    Ok(ApiResponse::ok(updated))
}
/// DELETE /api/users/{id}（仅管理员）
pub async fn remove(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> Result<ApiResponse<serde_json::Value>, AppError> {
    user_service::delete(&state.pool, &user, &id).await?;
    Ok(ApiResponse::ok(serde_json::json!({ "deleted": id })))
}