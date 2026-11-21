//! 版本与升级检查的数据结构。

use serde::{Deserialize, Serialize};

/// 部署方写入的更新清单（data/update.json）
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateManifest {
    pub version: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub published_at: Option<i64>,
}

/// GET /api/system/version 的响应
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    /// 当前运行版本
    pub current: String,
    /// 可用的最新版本（未配置更新清单时等于 current）
    pub latest: String,
    /// 是否存在更新
    pub update_available: bool,
    /// 更新说明
    pub notes: String,
    /// 清单发布时间
    pub published_at: Option<i64>,
    /// 清单来源：`manifest`（读取本地 update.json）或 `none`
    pub source: String,
    /// 官网仓库（owner/name）
    pub repo: String,
    /// 官网地址
    pub homepage: String,
    /// 发布页（更新日志）
    pub releases_url: String,
    /// 供前端查询最新版本的 GitHub API 地址
    pub check_url: String,
    /// 本次检查时间
    pub checked_at: i64,
    /// 升级动作提示（按平台给出）
    pub upgrade_hint: Vec<String>,
}