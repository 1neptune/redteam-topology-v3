//! 版本信息：只报表当前运行的版本与仓库地址，不做任何联网检查。
//!
//! 升级一律走命令行（`scripts/upgrade.sh`）或替换二进制，界面上不提供
//! 「检查更新 / 一键更新」——容器里本来也无法重建自身，徒增困惑。

use crate::config::Config;
use crate::error::AppError;
use crate::model::system::VersionInfo;

/// 组装版本信息
pub fn version_info(cfg: &Config) -> Result<VersionInfo, AppError> {
    let repo = cfg.update_repo.trim().to_string();
    Ok(VersionInfo {
        current: env!("CARGO_PKG_VERSION").to_string(),
        repo: repo.clone(),
        homepage: format!("https://github.com/{repo}"),
        releases_url: format!("https://github.com/{repo}/releases"),
        upgrade_hint: upgrade_hint(),
        checked_at: crate::service::now_ms(),
    })
}

/// 升级方式提示：全部是命令行步骤
fn upgrade_hint() -> Vec<String> {
    vec![
        "Docker：在项目目录执行 ./scripts/upgrade.sh（备份、构建、重启、失败回滚）".to_string(),
        "Docker 指定版本：./scripts/upgrade.sh v4.0.0".to_string(),
        "二进制：停止进程，替换 redteam-atlas，再启动（数据目录保留，迁移自动执行）".to_string(),
        "查看版本：./scripts/upgrade.sh --check".to_string(),
    ]
}