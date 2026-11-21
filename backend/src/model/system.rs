use serde::Serialize;

/// 版本信息（只读，不做联网检查）
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    /// 当前运行版本
    pub current: String,
    /// 仓库 owner/name
    pub repo: String,
    /// 仓库主页
    pub homepage: String,
    /// 发布页
    pub releases_url: String,
    /// 命令行升级方式
    pub upgrade_hint: Vec<String>,
    /// 本次查询时间
    pub checked_at: i64,
}