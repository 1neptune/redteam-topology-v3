//! 版本检查业务：读取部署方写入的更新清单并与当前版本比较。
//! 不主动访问外网，升级信息由升级脚本 / CI 写入 update.json。

use crate::config::Config;
use crate::error::AppError;
use crate::model::system::{UpdateManifest, VersionInfo};

/// 把 "3.10.2" 这类版本号解析成可比较的整数段
fn parse_version(raw: &str) -> Vec<u64> {
    raw.trim()
        .trim_start_matches('v')
        .split(['.', '-', '+'])
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect()
}

/// a 是否比 b 新
fn is_newer(a: &str, b: &str) -> bool {
    let left = parse_version(a);
    let right = parse_version(b);
    let len = left.len().max(right.len());
    for index in 0..len {
        let l = *left.get(index).unwrap_or(&0);
        let r = *right.get(index).unwrap_or(&0);
        if l != r {
            return l > r;
        }
    }
    false
}

/// 读取更新清单；文件不存在或格式损坏时返回 None（不阻断接口）
fn read_manifest(cfg: &Config) -> Option<UpdateManifest> {
    let raw = std::fs::read_to_string(&cfg.update_file).ok()?;
    // Windows 记事本与 PowerShell 的 UTF-8 会带 BOM，解析前去掉
    let cleaned = raw.trim_start_matches('\u{feff}').trim();
    match serde_json::from_str::<UpdateManifest>(cleaned) {
        Ok(manifest) if !manifest.version.trim().is_empty() => Some(manifest),
        Ok(_) => None,
        Err(err) => {
            tracing::warn!(error = %err, path = %cfg.update_file.display(), "更新清单解析失败");
            None
        }
    }
}

/// 组装版本信息
pub fn version_info(cfg: &Config) -> Result<VersionInfo, AppError> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let checked_at = crate::service::now_ms();
    let manifest = read_manifest(cfg);

    let (latest, notes, published_at, source, update_available) = match manifest {
        Some(item) => {
            let newer = is_newer(&item.version, &current);
            (item.version, item.notes, item.published_at, "manifest".to_string(), newer)
        }
        None => (
            current.clone(),
            String::new(),
            None,
            "none".to_string(),
            false,
        ),
    };

    let repo = cfg.update_repo.trim().to_string();
    let homepage = format!("https://github.com/{repo}");
    Ok(VersionInfo {
        current: current.clone(),
        latest,
        update_available,
        notes,
        published_at,
        source,
        repo: repo.clone(),
        homepage,
        releases_url: format!("https://github.com/{repo}/releases"),
        check_url: format!("https://api.github.com/repos/{repo}/releases/latest"),
        checked_at,
        upgrade_hint: upgrade_hint(&current),
    })
}

/// 升级步骤提示（Windows / Linux / Docker）
fn upgrade_hint(current: &str) -> Vec<String> {
    vec![
        "Windows：停止 atlas.exe，替换为新的 redteam-atlas.exe，再启动（数据目录保留，迁移自动执行）".to_string(),
        "Linux：systemctl stop atlas，覆盖 /usr/local/bin/redteam-atlas，systemctl start atlas".to_string(),
        "Docker：docker compose pull && docker compose up -d（镜像内已含前端与迁移脚本）".to_string(),
        format!("升级后访问「检查更新」确认版本从 {current} 变为目标版本"),
    ]
}