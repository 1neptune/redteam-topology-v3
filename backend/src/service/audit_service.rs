//! 审计业务：把变更写入库、读出并解析 diff。

use sqlx::{SqliteConnection, SqlitePool};

use crate::auth::AuthUser;
use crate::db::{audit_db, new_id};
use crate::error::AppError;
use crate::model::misc::AuditRow;
use crate::model::{AuditRecord, Change, FieldChange};
use crate::service::now_ms;

/// 单条记录 detail 的最大字节数，超出截断避免撑爆存储
const MAX_DETAIL_BYTES: usize = 12 * 1024;

/// 把 diff 结果批量写入审计（同一事务）
pub async fn record_changes(
    conn: &mut SqliteConnection,
    project_id: &str,
    actor: &AuthUser,
    changes: &[Change],
) -> Result<(), AppError> {
    let at = now_ms();
    for change in changes.iter().take(80) {
        let detail = serde_json::to_string(&change.changes).unwrap_or_else(|_| "[]".to_string());
        let detail = if detail.len() > MAX_DETAIL_BYTES {
            None
        } else {
            Some(detail)
        };
        audit_db::insert(
            conn,
            &new_id("a_"),
            Some(project_id),
            Some(&actor.id),
            &actor.display_name,
            &change.action,
            &change.target_type,
            &change.target_id,
            &change.target_label,
            &change.summary,
            detail.as_deref(),
            at,
        )
        .await?;
    }
    Ok(())
}

/// 写入一条非拓扑类审计（授权、项目资料等）
#[allow(clippy::too_many_arguments)]
pub async fn record_event(
    conn: &mut SqliteConnection,
    project_id: Option<&str>,
    actor: &AuthUser,
    action: &str,
    target_type: &str,
    target_id: &str,
    target_label: &str,
    summary: &str,
) -> Result<(), AppError> {
    audit_db::insert(
        conn,
        &new_id("a_"),
        project_id,
        Some(&actor.id),
        &actor.display_name,
        action,
        target_type,
        target_id,
        target_label,
        summary,
        None,
        now_ms(),
    )
    .await
}

/// 解析 detail 为字段变更列表
fn parse_detail(detail: Option<&str>) -> Vec<FieldChange> {
    match detail {
        Some(raw) if !raw.is_empty() => serde_json::from_str::<Vec<FieldChange>>(raw).unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// 行 → 对外结构
fn to_record(row: AuditRow) -> AuditRecord {
    let changes = parse_detail(row.detail.as_deref());
    AuditRecord {
        id: row.id,
        project_id: row.project_id,
        user_name: row.user_name,
        action: row.action,
        target_type: row.target_type,
        target_id: row.target_id,
        target_label: row.target_label,
        summary: row.summary,
        changes,
        created_at: row.created_at,
    }
}

/// 项目审计
pub async fn list_by_project(
    pool: &SqlitePool,
    project_id: &str,
    limit: i64,
    before: Option<i64>,
) -> Result<Vec<AuditRecord>, AppError> {
    let limit = limit.clamp(1, 200);
    let rows = audit_db::list_by_project(pool, project_id, limit, before).await?;
    Ok(rows.into_iter().map(to_record).collect())
}

/// 全局审计（管理员）
pub async fn list_all(
    pool: &SqlitePool,
    limit: i64,
    before: Option<i64>,
) -> Result<Vec<AuditRecord>, AppError> {
    let limit = limit.clamp(1, 300);
    let rows = audit_db::list_all(pool, limit, before).await?;
    Ok(rows.into_iter().map(to_record).collect())
}