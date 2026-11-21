//! 全局审计接口（管理员）。

use axum::extract::{Query, State};

use crate::auth::AuthUser;
use crate::error::AppError;
use crate::model::misc::AuditQuery;
use crate::model::AuditRecord;
use crate::response::ApiResponse;
use crate::service::audit_service;
use crate::state::AppState;

/// GET /api/audit
pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Query(query): Query<AuditQuery>,
) -> Result<ApiResponse<Vec<AuditRecord>>, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("仅管理员可查看全局审计".to_string()));
    }
    let list = audit_service::list_all(&state.pool, query.limit.unwrap_or(80), query.before).await?;
    Ok(ApiResponse::ok(list))
}