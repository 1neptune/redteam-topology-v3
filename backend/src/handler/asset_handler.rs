//! 组件类型与图标上传接口。

use axum::extract::{Path, State, Query};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::auth::AuthUser;
use crate::error::AppError;
use crate::handler::ApiJson;
use crate::model::misc::{AssetTypeRequest, UploadRequest, UploadResult};
use crate::model::AssetType;
use crate::response::ApiResponse;
use crate::service::asset_service;
use crate::state::AppState;

/// GET /api/asset-types
pub async fn list_types(State(state): State<AppState>) -> Result<ApiResponse<Vec<AssetType>>, AppError> {
    let list = asset_service::list(&state.pool).await?;
    Ok(ApiResponse::ok(list))
}

/// POST /api/asset-types
pub async fn create_type(
    State(state): State<AppState>,
    user: AuthUser,
    ApiJson(req): ApiJson<AssetTypeRequest>,
) -> Result<ApiResponse<AssetType>, AppError> {
    let created = asset_service::create(&state.pool, &user, req).await?;
    Ok(ApiResponse::ok(created))
}

/// PATCH /api/asset-types/{slug}
pub async fn update_type(
    State(state): State<AppState>,
    user: AuthUser,
    Path(slug): Path<String>,
    ApiJson(req): ApiJson<AssetTypeRequest>,
) -> Result<ApiResponse<AssetType>, AppError> {
    let updated = asset_service::update(&state.pool, &user, &slug, req).await?;
    Ok(ApiResponse::ok(updated))
}

/// DELETE /api/asset-types/{slug}
pub async fn delete_type(
    State(state): State<AppState>,
    user: AuthUser,
    Path(slug): Path<String>,
) -> Result<ApiResponse<()>, AppError> {
    asset_service::delete(&state.pool, &user, &slug).await?;
    Ok(crate::response::ok_empty())
}

/// POST /api/uploads
pub async fn upload(
    State(state): State<AppState>,
    user: AuthUser,
    ApiJson(req): ApiJson<UploadRequest>,
) -> Result<ApiResponse<UploadResult>, AppError> {
    let result = asset_service::save_upload(&state.pool, &state.cfg, &user, req).await?;
    Ok(ApiResponse::ok(result))
}

/// GET /api/pub/uploads/{id}?sig=...
///
/// 正文里的图片地址：用短签名代替登录态，浏览器可直接加载并缓存。
/// 签名无效与内容不存在返回同一个 404，避免探测。
pub async fn get_public_upload(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<crate::model::UploadQuery>,
) -> Result<Response, AppError> {
    if !crate::service::asset_service::verify_upload(&state.cfg.master_key, &id, &query.sig) {
        return Err(AppError::NotFound("图片不存在".to_string()));
    }
    let (mime, bytes) = asset_service::read_upload(&state.pool, &state.cfg, &id).await?;
    Ok(image_response(mime, bytes))
}

/// GET /api/uploads/{id}（需登录）
/// 返回图片二进制；通过 CSP 与 nosniff 阻止 SVG 内的脚本在浏览器执行。
pub async fn get_upload(
    State(state): State<AppState>,
    _user: AuthUser,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let (mime, bytes) = asset_service::read_upload(&state.pool, &state.cfg, &id).await?;
    Ok(image_response(mime, bytes))
}

/// 图片响应：类型、防嗅探、沙箱 CSP、长缓存
fn image_response(mime: String, bytes: Vec<u8>) -> Response {
    let mut headers = HeaderMap::new();
    let content_type = HeaderValue::from_str(&mime).unwrap_or(HeaderValue::from_static("image/png"));
    headers.insert(header::CONTENT_TYPE, content_type);
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; style-src 'unsafe-inline'; sandbox"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );

    (StatusCode::OK, headers, bytes).into_response()
}