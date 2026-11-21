//! audit_log 表的 SQL 操作。

use sqlx::{SqliteConnection, SqlitePool};

use crate::error::AppError;
use crate::model::misc::AuditRow;

/// 写入一条审计（与业务写操作同事务）
#[allow(clippy::too_many_arguments)]
pub async fn insert(
    conn: &mut SqliteConnection,
    id: &str,
    project_id: Option<&str>,
    user_id: Option<&str>,
    user_name: &str,
    action: &str,
    target_type: &str,
    target_id: &str,
    target_label: &str,
    summary: &str,
    detail: Option<&str>,
    created_at: i64,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO audit_log (id, project_id, user_id, user_name, action, target_type,
                                target_id, target_label, summary, detail, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(project_id)
    .bind(user_id)
    .bind(user_name)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(target_label)
    .bind(summary)
    .bind(detail)
    .bind(created_at)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// 项目内审计（按时间倒序，支持游标分页）
pub async fn list_by_project(
    pool: &SqlitePool,
    project_id: &str,
    limit: i64,
    before: Option<i64>,
) -> Result<Vec<AuditRow>, AppError> {
    let rows = sqlx::query_as::<_, AuditRow>(
        "SELECT id, project_id, user_name, action, target_type, target_id, target_label,
                summary, detail, created_at
         FROM audit_log
         WHERE project_id = ? AND (? IS NULL OR created_at < ?)
         ORDER BY created_at DESC
         LIMIT ?",
    )
    .bind(project_id)
    .bind(before)
    .bind(before)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 全局审计（管理员）
pub async fn list_all(
    pool: &SqlitePool,
    limit: i64,
    before: Option<i64>,
) -> Result<Vec<AuditRow>, AppError> {
    let rows = sqlx::query_as::<_, AuditRow>(
        "SELECT id, project_id, user_name, action, target_type, target_id, target_label,
                summary, detail, created_at
         FROM audit_log
         WHERE (? IS NULL OR created_at < ?)
         ORDER BY created_at DESC
         LIMIT ?",
    )
    .bind(before)
    .bind(before)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}