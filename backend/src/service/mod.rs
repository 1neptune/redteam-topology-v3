//! 业务逻辑层：校验、权限判定、事务编排。禁止在这里拼 SQL 语句字符串。

pub mod asset_service;
pub mod audit_service;
pub mod auth_service;
pub mod graph_service;
pub mod project_log;
pub mod run_log;
pub mod project_service;
pub mod system_service;
pub mod upgrade_service;
pub mod user_service;

/// 当前毫秒时间戳（全库统一时间单位）
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 裁剪字符串到最大字符数（按字符而非字节，避免切断中文）
pub fn clamp(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    value.chars().take(max_chars).collect()
}

/// 去掉首尾空白并裁断
pub fn tidy(value: &str, max_chars: usize) -> String {
    clamp(value.trim(), max_chars)
}

/// 校验用户名：3-32 位，字母数字下划线连字符
pub fn valid_username(name: &str) -> bool {
    let len = name.chars().count();
    (3..=32).contains(&len)
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// 校验口令长度
pub fn valid_password(password: &str) -> bool {
    let count = password.chars().count();
    (8..=128).contains(&count)
        && password.chars().any(|c| c.is_ascii_uppercase())
        && password.chars().any(|c| c.is_ascii_lowercase())
}

/// 口令规则的人类可读说明
pub const PASSWORD_RULE: &str = "口令至少 8 位，且同时包含大写和小写字母";

/// 允许的角色值
pub fn valid_role(role: &str) -> bool {
    matches!(role, "admin" | "member")
}

/// 允许的项目权限值
pub fn valid_permission(permission: &str) -> bool {
    matches!(permission, "owner" | "editor" | "viewer")
}

/// 漏洞条目 id：`v_` + 32 位十六进制
pub fn valid_vuln_id(id: &str) -> bool {
    id.len() == 34 && id.starts_with("v_") && id[2..].chars().all(|c| c.is_ascii_hexdigit())
}

/// 校验十六进制颜色
pub fn valid_color(color: &str) -> bool {
    let c = color.trim();
    if c.len() != 7 || !c.starts_with('#') {
        return false;
    }
    c[1..].chars().all(|ch| ch.is_ascii_hexdigit())
}