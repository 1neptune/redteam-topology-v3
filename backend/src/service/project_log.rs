//! 项目日志：把每次写入落成可 grep 的行文本，方便事后追一条攻击路径是怎么改出来的。
//!
//! 与审计表的分工：审计表在库里，供界面「审计记录」查询；
//! 这里写文件，供运维直接 `tail -f`、打包带走或投喂给日志系统。
//! 一行一条，字段用 Tab 分隔，避免中文内容里的分隔符歧义。

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::config::Config;

/// 日志目录：`ATLAS_LOG_DIR`，默认落在数据目录下的 `logs/`
pub fn log_dir(cfg: &Config) -> PathBuf {
    if let Ok(value) = std::env::var("ATLAS_LOG_DIR") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    cfg.update_file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("./data"))
        .join("logs")
}

/// 本地时区偏移（小时）：`ATLAS_TZ_OFFSET`，默认 +8
fn tz_offset_hours() -> i64 {
    std::env::var("ATLAS_TZ_OFFSET")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .unwrap_or(8)
}

/// 时间戳 → `2026-11-20 13:58:12`（本地时间，便于直接对照界面上的时间）
fn stamp(ms: i64) -> String {
    let secs = ms / 1000 + tz_offset_hours() * 3600;
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (hour, minute, second) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // 从 1970-01-01 起算，按民用历法换算
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

/// 天序号 → (年, 月, 日)，Howard Hinnant 的 civil_from_days
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// 把一行日志追加到 `projects.log`
pub fn record(cfg: &Config, project_id: &str, project_name: &str, actor: &str, action: &str, detail: &str) {
    let dir = log_dir(cfg);
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("projects.log");
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };
    // 中文内容里如果有 Tab 或换行会破坏列对齐，统一替换成空格
    let clean = |text: &str| text.replace(['\t', '\n', '\r'], " ");
    let line = format!(
        "{}\t{}\t{}\t{}\t{}\t{}\n",
        stamp(crate::service::now_ms()),
        clean(project_id),
        clean(project_name),
        clean(actor),
        clean(action),
        clean(detail)
    );
    let _ = file.write_all(line.as_bytes());
}