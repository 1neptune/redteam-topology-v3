//! 组件类型、审计、在线状态等杂项结构体。

use serde::{Deserialize, Serialize};

/// asset_types 表实体
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AssetType {
    pub slug: String,
    pub label: String,
    pub color: String,
    pub icon_url: Option<String>,
    pub builtin: bool,
    pub created_by: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 新建/修改组件请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetTypeRequest {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub icon_url: Option<String>,
}

/// 上传请求（base64）
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadRequest {
    pub filename: String,
    pub mime: String,
    pub data_base64: String,
}

/// GET /api/pub/uploads/{id} 的查询参数
#[derive(Debug, Deserialize)]
pub struct UploadQuery {
    #[serde(default)]
    pub sig: String,
}

/// 上传结果
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadResult {
    pub id: String,
    pub url: String,
    pub bytes: usize,
    pub mime: String,
}

/// 审计记录
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditRecord {
    pub id: String,
    pub project_id: Option<String>,
    pub user_name: String,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub target_label: String,
    pub summary: String,
    pub changes: Vec<super::graph::FieldChange>,
    pub created_at: i64,
}

/// audit_log 原始行
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AuditRow {
    pub id: String,
    pub project_id: Option<String>,
    pub user_name: String,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub target_label: String,
    pub summary: String,
    pub detail: Option<String>,
    pub created_at: i64,
}

/// 在线成员
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PresenceEntry {
    pub user_id: String,
    pub user_name: String,
    pub cursor_x: f64,
    pub cursor_y: f64,
    pub seen_at: i64,
}

/// 心跳请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresencePing {
    #[serde(default)]
    pub cursor_x: f64,
    #[serde(default)]
    pub cursor_y: f64,
}

/// 审计查询参数
#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub before: Option<i64>,
}

/// 项目列表查询参数
#[derive(Debug, Deserialize)]
pub struct ProjectQuery {
    #[serde(default)]
    pub q: Option<String>,
}