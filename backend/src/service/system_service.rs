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

/// 后端代查 GitHub 最新发布的结果，带 5 分钟缓存。
///
/// 前端不再直连 api.github.com：浏览器所在网络经常连不上 GitHub，
/// 而服务器通常可以（或至少能通过 update.json 拿到）。
fn github_latest(repo: &str) -> Option<(String, String, i64)> {
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    struct Cached {
        at: Instant,
        value: Option<(String, String, i64)>,
    }

    static CACHE: OnceLock<Mutex<Cached>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| {
        Mutex::new(Cached {
            at: Instant::now() - Duration::from_secs(3600),
            value: None,
        })
    });

    if let Ok(guard) = cache.lock() {
        if guard.at.elapsed() < Duration::from_secs(300) {
            return guard.value.clone();
        }
    }

    let value = fetch_github_latest(repo);
    if let Ok(mut guard) = cache.lock() {
        guard.at = Instant::now();
        guard.value = value.clone();
    }
    value
}

/// 真正去打一次 GitHub API；任何失败都返回 None，不抛错
fn fetch_github_latest(repo: &str) -> Option<(String, String, i64)> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let response = ureq::get(&url)
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", "redteam-atlas")
        .timeout(std::time::Duration::from_secs(8))
        .call()
        .map_err(|err| tracing::debug!(error = %err, "查询 GitHub 最新版本失败"))
        .ok()?;
    let body = response
        .into_json::<serde_json::Value>()
        .map_err(|err| tracing::debug!(error = %err, "解析 GitHub 发布信息失败"))
        .ok()?;

    let tag = body.get("tag_name")?.as_str()?.trim().to_string();
    let version = tag.trim_start_matches(['v', 'V']).to_string();
    if version.is_empty() {
        return None;
    }
    let notes = body
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .chars()
        .take(4000)
        .collect::<String>();
    // published_at 是 ISO 时间，只取日期部分够用
    let published_at = body
        .get("published_at")
        .and_then(|v| v.as_str())
        .and_then(|raw| raw.get(0..10))
        .and_then(|date| {
            let mut parts = date.split('-');
            let y: i64 = parts.next()?.parse().ok()?;
            let m: i64 = parts.next()?.parse().ok()?;
            let d: i64 = parts.next()?.parse().ok()?;
            Some(days_to_ms(y, m, d))
        })
        .unwrap_or(0);

    Some((version, notes, published_at))
}

/// 粗略把年月日换算成毫秒时间戳，只用于展示
fn days_to_ms(year: i64, month: i64, day: i64) -> i64 {
    // 从 1970 起算的近似天数：够精确到日即可
    let mut days = (year - 1970) * 365 + (year - 1969) / 4;
    let month_days = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for index in 0..(month - 1).clamp(0, 11) as usize {
        days += month_days[index];
    }
    if month > 2 && year % 4 == 0 {
        days += 1;
    }
    days += day - 1;
    days * 86_400_000
}
/// 组装版本信息
pub fn version_info(cfg: &Config) -> Result<VersionInfo, AppError> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let checked_at = crate::service::now_ms();
    let manifest = read_manifest(cfg);

    let repo_for_check = cfg.update_repo.trim().to_string();
    // 来源优先级：本地 update.json（服务器上由升级脚本写入，最稳）→ GitHub API
    let (latest, notes, published_at, source, update_available) = match manifest {
        Some(item) => {
            let newer = is_newer(&item.version, &current);
            (item.version, item.notes, item.published_at, "manifest".to_string(), newer)
        }
        None => match github_latest(&repo_for_check) {
            Some((version, notes, published_at)) => {
                let newer = is_newer(&version, &current);
                (
                    version,
                    notes,
                    (published_at > 0).then_some(published_at),
                    "github".to_string(),
                    newer,
                )
            }
            None => (current.clone(), String::new(), None, "none".to_string(), false),
        },
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