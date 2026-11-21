//! 项目与成员相关结构体。

use serde::{Deserialize, Serialize};

use super::graph::Graph;

/// projects 表实体（含加密列）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProjectRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub owner_id: String,
    pub version: i64,
    pub node_count: i64,
    pub edge_count: i64,
    pub data_cipher: Vec<u8>,
    pub data_iv: Vec<u8>,
    pub data_tag: Vec<u8>,
    pub created_at: i64,
    pub updated_at: i64,
    pub updated_by: String,
    pub status: String,
}

/// 列表项
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub owner_id: String,
    pub version: i64,
    pub node_count: i64,
    pub edge_count: i64,
    pub permission: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub updated_by: String,
    /// active（进行中）/ done（已完成）
    pub status: String,
}

/// 详情（含解密拓扑）
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetail {
    pub project: ProjectSummary,
    pub permission: String,
    pub graph: Graph,
}

/// 成员条目
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct MemberEntry {
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    pub permission: String,
    pub granted_at: i64,
}

/// 保存结果
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    pub version: i64,
    pub node_count: i64,
    pub edge_count: i64,
    pub changes: usize,
    pub updated_at: i64,
}

/// 新建项目
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub data: Option<Graph>,
    /// 新建时的初始状态，缺省按进行中处理
    #[serde(default)]
    #[allow(dead_code)]
    pub status: Option<String>,
}

/// 保存拓扑
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProjectRequest {
    pub data: Graph,
    #[serde(default)]
    pub base_version: Option<i64>,
}

/// 改项目资料
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

/// 授权
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrantRequest {
    pub user_id: String,
    pub permission: String,
}

/// 导入
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRequest {
    pub project: ImportProject,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProject {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub data: Graph,
}

/// 导出结构
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportBundle {
    pub format: String,
    pub schema_version: i32,
    pub exported_at: i64,
    pub exported_by: String,
    pub project: ExportProject,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProject {
    pub name: String,
    pub description: String,
    pub data: Graph,
}