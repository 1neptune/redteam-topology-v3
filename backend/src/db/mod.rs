//! 数据库访问层：只写 SQL 与编解码，不写业务判断。
//! 所有查询使用参数绑定（`?` + `bind`），杜绝拼接注入。

pub mod asset_db;
pub mod audit_db;
pub mod crypto;
pub mod project_db;
pub mod user_db;

/// 生成带前缀的随机 ID（便于日志辨认实体类型）
pub fn new_id(prefix: &str) -> String {
    format!("{prefix}{}", uuid::Uuid::new_v4().simple())
}