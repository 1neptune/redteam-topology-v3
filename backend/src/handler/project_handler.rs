//! 项目、成员、在线状态、审计接口。

use axum::extract::{Path, Query, State};

use crate::auth::AuthUser;
use crate::error::AppError;
use crate::handler::ApiJson;
use crate::model::misc::{AuditQuery, PresencePing, ProjectQuery};
use crate::model::project::{
    CreateProjectRequest, ExportBundle, GrantRequest, ImportRequest, ProjectDetail, ProjectSummary,
    SaveProjectRequest, SaveResult, UpdateProjectRequest,
};
use crate::model::{AuditRecord, MemberEntry, PresenceEntry};
use crate::response::{ok_empty, ApiResponse};
use crate::service::{audit_service, project_service};
use crate::state::AppState;

/// GET /api/projects
pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Query(query): Query<ProjectQuery>,
) -> Result<ApiResponse<Vec<ProjectSummary>>, AppError> {
    let keyword = query.q.unwrap_or_default();
    let list = project_service::list(&state.pool, &user, &keyword).await?;
    Ok(ApiResponse::ok(list))
}

/// POST /api/projects
pub async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    ApiJson(req): ApiJson<CreateProjectRequest>,
) -> Result<ApiResponse<ProjectSummary>, AppError> {
    let created = project_service::create(&state.pool, &state.cfg, &user, req).await?;
    Ok(ApiResponse::ok(created))
}

/// GET /api/projects/{id}
pub async fn detail(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> Result<ApiResponse<ProjectDetail>, AppError> {
    let detail = project_service::detail(&state.pool, &state.cfg, &user, &id).await?;
    Ok(ApiResponse::ok(detail))
}

/// PUT /api/projects/{id}
pub async fn save(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    ApiJson(req): ApiJson<SaveProjectRequest>,
) -> Result<ApiResponse<SaveResult>, AppError> {
    let result = project_service::save(&state.pool, &state.cfg, &user, &id, req).await?;
    Ok(ApiResponse::ok(result))
}

/// PATCH /api/projects/{id}
pub async fn patch(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    ApiJson(req): ApiJson<UpdateProjectRequest>,
) -> Result<ApiResponse<ProjectSummary>, AppError> {
    let summary = project_service::update_meta(&state.pool, &user, &id, req).await?;
    Ok(ApiResponse::ok(summary))
}

/// DELETE /api/projects/{id}
pub async fn delete(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> Result<ApiResponse<()>, AppError> {
    project_service::delete(&state.pool, &user, &id).await?;
    Ok(ok_empty())
}

/// POST /api/projects/{id}/duplicate
pub async fn duplicate(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> Result<ApiResponse<ProjectSummary>, AppError> {
    let summary = project_service::duplicate(&state.pool, &state.cfg, &user, &id).await?;
    Ok(ApiResponse::ok(summary))
}

/// GET /api/projects/{id}/export
pub async fn export(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> Result<ApiResponse<ExportBundle>, AppError> {
    let bundle = project_service::export(&state.pool, &state.cfg, &user, &id).await?;
    Ok(ApiResponse::ok(bundle))
}

/// POST /api/projects/import
pub async fn import(
    State(state): State<AppState>,
    user: AuthUser,
    ApiJson(req): ApiJson<ImportRequest>,
) -> Result<ApiResponse<ProjectSummary>, AppError> {
    let summary = project_service::import(&state.pool, &state.cfg, &user, req).await?;
    Ok(ApiResponse::ok(summary))
}

/// GET /api/projects/{id}/members
pub async fn members(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> Result<ApiResponse<Vec<MemberEntry>>, AppError> {
    let list = project_service::members(&state.pool, &user, &id).await?;
    Ok(ApiResponse::ok(list))
}

/// POST /api/projects/{id}/members
pub async fn grant(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    ApiJson(req): ApiJson<GrantRequest>,
) -> Result<ApiResponse<Vec<MemberEntry>>, AppError> {
    let list = project_service::grant(&state.pool, &user, &id, req).await?;
    Ok(ApiResponse::ok(list))
}

/// DELETE /api/projects/{id}/members/{user_id}
pub async fn revoke(
    State(state): State<AppState>,
    user: AuthUser,
    Path((id, user_id)): Path<(String, String)>,
) -> Result<ApiResponse<Vec<MemberEntry>>, AppError> {
    let list = project_service::revoke(&state.pool, &user, &id, &user_id).await?;
    Ok(ApiResponse::ok(list))
}

/// GET /api/projects/{id}/audit
pub async fn audit(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Query(query): Query<AuditQuery>,
) -> Result<ApiResponse<Vec<AuditRecord>>, AppError> {
    // 先校验项目可见性，避免通过审计接口探测项目
    project_service::members(&state.pool, &user, &id).await?;
    let list = audit_service::list_by_project(&state.pool, &id, query.limit.unwrap_or(40), query.before).await?;
    Ok(ApiResponse::ok(list))
}

/// GET /api/projects/{id}/presence
pub async fn presence(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> Result<ApiResponse<Vec<PresenceEntry>>, AppError> {
    let list = project_service::presence(&state.pool, &user, &id).await?;
    Ok(ApiResponse::ok(list))
}

/// POST /api/projects/{id}/presence
pub async fn ping(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    ApiJson(req): ApiJson<PresencePing>,
) -> Result<ApiResponse<()>, AppError> {
    project_service::ping(&state.pool, &user, &id, req.cursor_x, req.cursor_y).await?;
    Ok(ok_empty())
}