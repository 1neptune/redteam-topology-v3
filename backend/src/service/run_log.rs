//! 运行日志：接口访问与错误两类，落在数据目录的 `logs/` 下。
//!
//! 两类文件各管一件事：
//! - `access.log`：每一次 `/api` 请求，含时间、来源 IP、接口、请求体、状态码、响应体；
//! - `error.log`：5xx 与解析失败等异常，含时间、级别、接口与错误内容。
//!
//! 都是追加写的纯文本（Tab 分隔、单行一条），运维可以直接 `tail -f`。

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::Config;

/// 单条日志里请求体与响应体的最大字符数，超出截断
pub const BODY_LIMIT: usize = 600;

/// 日志目录：`ATLAS_LOG_DIR`，默认落在数据目录下的 `logs/`
pub fn log_dir(cfg: &Config) -> PathBuf {
    if let Ok(value) = std::env::var("ATLAS_LOG_DIR") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    cfg.upload_dir
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

/// 当前时间 → `2026-11-20 15:24:36`
pub fn stamp_now() -> String {
    stamp(now_ms())
}

/// 毫秒时间戳 → 本地可读时间
pub fn stamp(ms: i64) -> String {
    let secs = ms / 1000 + tz_offset_hours() * 3600;
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (hour, minute, second) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 天序号 → (年, 月, 日)
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

/// 抹掉请求/响应里的凭据：口令、令牌与密钥字段一律替换成 ***
pub fn redact(text: &str) -> String {
    // 逐个敏感键处理：形如 "password":"xxx" 的值整体替换
    let mut current = text.to_string();
    for key in ["password", "token", "secret", "authorization", "newPassword", "next"] {
        current = mask_key(&current, key);
    }
    current
}

/// 把 `"key":"..."` 里的值换成 ***
fn mask_key(text: &str, key: &str) -> String {
    let needle = format!("\"{key}\":\"");
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find(&needle) {
        let (before, after) = rest.split_at(pos + needle.len());
        result.push_str(before);
        // 找到值的结束引号（跳过转义）
        let bytes: Vec<char> = after.chars().collect();
        let mut end = None;
        let mut escaped = false;
        let mut consumed = 0;
        for (i, ch) in bytes.iter().enumerate() {
            consumed += ch.len_utf8();
            if escaped {
                escaped = false;
                continue;
            }
            if *ch == '\\' {
                escaped = true;
                continue;
            }
            if *ch == '"' {
                end = Some(i);
                break;
            }
        }
        match end {
            Some(_) => {
                // consumed 已越过结尾引号，这里补回来，日志里的 JSON 仍可读
                result.push_str("***\"");
                rest = &after[consumed..];
            }
            None => {
                result.push_str(after);
                return result;
            }
        }
    }
    result.push_str(rest);
    result
}

/// 压成单行并截断，避免换行破坏一行一条的约定
pub fn flatten(text: &str) -> String {
    let single = text.replace(['\t', '\n', '\r'], " ");
    let trimmed = single.trim();
    if trimmed.chars().count() <= BODY_LIMIT {
        return trimmed.to_string();
    }
    let cut: String = trimmed.chars().take(BODY_LIMIT).collect();
    format!("{cut}…")
}

/// 追加一行到指定日志文件
pub fn append(cfg: &Config, file: &str, line: &str) {
    let dir = log_dir(cfg);
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join(file);
    let Ok(mut handle) = OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };
    let _ = handle.write_all(line.as_bytes());
}

/// 记一条接口访问
pub fn access(cfg: &Config, ip: &str, method: &str, path: &str, request: &str, status: u16, response: &str) {
    let line = format!(
        "{}\t{}\t{} {}\t{}\t{}\t{}\n",
        stamp_now(),
        flatten(ip),
        method,
        path,
        flatten(&redact(request)),
        status,
        flatten(&redact(response))
    );
    append(cfg, "access.log", &line);
}

/// 记一条错误
pub fn error(cfg: &Config, ip: &str, method: &str, path: &str, status: u16, message: &str) {
    let line = format!(
        "{}\tERROR\t{}\t{} {}\t{}\t{}\n",
        stamp_now(),
        flatten(ip),
        method,
        path,
        status,
        flatten(message)
    );
    append(cfg, "error.log", &line);
}

/// 读取日志文件的最后 `limit` 行，返回结构化记录
pub fn tail(cfg: &Config, file: &str, limit: usize) -> Vec<Vec<String>> {
    let path = log_dir(cfg).join(file);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let mut lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.len() > limit {
        lines = lines.split_off(lines.len() - limit);
    }
    lines
        .into_iter()
        .map(|line| line.split('\t').map(|part| part.to_string()).collect())
        .collect()
}
/// 把 `tracing` 的 ERROR 事件写进 `error.log` 的 writer
///
/// 用全局路径而不是 `&Config`：tracing 的 writer 是 `'static`，拿不到请求上下文。
/// 路径在 `init_error_writer` 里设定一次。
static ERROR_LOG_PATH: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// 设定 error.log 的落盘位置，供 tracing writer 使用
pub fn init_error_writer(cfg: &Config) {
    let dir = log_dir(cfg);
    if std::fs::create_dir_all(&dir).is_ok() {
        let _ = ERROR_LOG_PATH.set(dir.join("error.log"));
    }
}

/// tracing 用的 writer
#[derive(Clone, Copy, Default)]
pub struct ErrorWriter;

impl std::io::Write for ErrorWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Some(path) = ERROR_LOG_PATH.get() {
            if let Ok(mut handle) = OpenOptions::new().create(true).append(true).open(path) {
                let _ = handle.write_all(buf);
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for ErrorWriter {
    type Writer = ErrorWriter;

    fn make_writer(&'a self) -> Self::Writer {
        ErrorWriter
    }
}