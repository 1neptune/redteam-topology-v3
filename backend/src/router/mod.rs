//! 路由注册：只负责路径与 handler 的映射，不写业务。
//! 除 `POST /api/auth/login` 外，所有 `/api` 接口都通过 `AuthUser` 提取器强制鉴权。

use std::path::PathBuf;

use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, patch, post};
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::handler::{asset_handler, audit_handler, auth_handler, export_handler, project_handler, system_handler, user_handler};
use crate::mcp;
use crate::middleware;
use crate::state::AppState;

/// 组装完整应用
pub fn build(state: AppState) -> Router {
    let api = Router::new()
        // 认证：仅 login 公开，其余均需登录
        .route("/auth/login", post(auth_handler::login))
        .route("/auth/logout", post(auth_handler::logout))
        .route("/auth/me", get(auth_handler::me))
        .route("/auth/password", post(auth_handler::change_password))
        .route("/auth/users", get(auth_handler::directory))
        // 运维探针（需登录）
        .route("/health", get(auth_handler::health))
        // 版本与更新检查
        .route("/system/version", get(system_handler::version))
        // 运行日志：接口访问与错误
        .route("/system/logs", get(system_handler::logs))
        // 用户管理（管理员）
        .route("/users", get(user_handler::list).post(user_handler::create))
        .route("/users/{id}", patch(user_handler::update).delete(user_handler::remove))
        // 项目
        .route(
            "/projects",
            get(project_handler::list).post(project_handler::create),
        )
        .route("/projects/import", post(project_handler::import))
        .route(
            "/projects/{id}",
            get(project_handler::detail)
                .put(project_handler::save)
                .patch(project_handler::patch)
                .delete(project_handler::delete),
        )
        .route("/projects/{id}/duplicate", post(project_handler::duplicate))
        .route("/projects/{id}/export", get(project_handler::export))
        .route(
            "/projects/{id}/members",
            get(project_handler::members).post(project_handler::grant),
        )
        .route(
            "/projects/{id}/members/{user_id}",
            delete(project_handler::revoke),
        )
        .route("/projects/{id}/audit", get(project_handler::audit))
        .route(
            "/projects/{id}/presence",
            get(project_handler::presence).post(project_handler::ping),
        )
        // 组件与上传（图标读取同样需要登录）
        .route(
            "/asset-types",
            get(asset_handler::list_types).post(asset_handler::create_type),
        )
        .route(
            "/asset-types/{slug}",
            patch(asset_handler::update_type).delete(asset_handler::delete_type),
        )
        .route("/uploads", post(asset_handler::upload))
        .route("/uploads/{id}", get(asset_handler::get_upload))
        // 正文插图：短签名代替登录态，浏览器可直接加载并缓存
        .route("/pub/uploads/{id}", get(asset_handler::get_public_upload))
        // 导出件：报告 docx/pdf、拓扑 png，文件名由服务端生成
        .route("/pub/exports/{name}", get(export_handler::get_export))
        // 全局审计
        .route("/audit", get(audit_handler::list));

    let logged = Router::new()
        .nest("/api", api)
        // MCP 端点保持 JSON-RPC 原格式；支持 Bearer 令牌与 HTTP Basic 两种鉴权
        .route("/mcp", post(mcp::handle).get(method_not_allowed))
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), middleware::record));

    Router::new()
        .merge(logged)
        .fallback_service(static_files())
        // 富文本详情内联 data URL 图片会放大请求体，放宽到 48MB（单图业务层另有压缩与校验）
        .layer(DefaultBodyLimit::max(48 * 1024 * 1024))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// GET /mcp 不是合法 MCP 传输方式
async fn method_not_allowed() -> (axum::http::StatusCode, &'static str) {
    (axum::http::StatusCode::METHOD_NOT_ALLOWED, "MCP supports POST only")
}

/// 前端静态资源（SPA 回退到 index.html）
fn static_files() -> ServeDir<ServeFile> {
    let dist = std::env::var("ATLAS_DIST_DIR").unwrap_or_else(|_| "./dist".to_string());
    let index: PathBuf = PathBuf::from(&dist).join("index.html");
    ServeDir::new(&dist).fallback(ServeFile::new(index))
}