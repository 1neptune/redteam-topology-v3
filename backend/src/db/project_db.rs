//! projects / project_members / presence 表的 SQL 操作。
//! 写操作接收 `&mut SqliteConnection`，便于 service 层放进同一事务。

use sqlx::{SqliteConnection, SqlitePool};

use crate::error::AppError;
use crate::model::{MemberEntry, PresenceEntry, ProjectRow, ProjectSummary};

/// 插入项目（含加密后的拓扑）
pub async fn insert(conn: &mut SqliteConnection, row: &ProjectRow) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO projects (id, name, description, owner_id, version, node_count, edge_count,
                               data_cipher, data_iv, data_tag, created_at, updated_at, updated_by, status)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&row.id)
    .bind(&row.name)
    .bind(&row.description)
    .bind(&row.owner_id)
    .bind(row.version)
    .bind(row.node_count)
    .bind(row.edge_count)
    .bind(&row.data_cipher)
    .bind(&row.data_iv)
    .bind(&row.data_tag)
    .bind(row.created_at)
    .bind(row.updated_at)
    .bind(&row.updated_by)
    .bind(&row.status)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// 读取项目行（含密文）
pub async fn find(pool: &SqlitePool, id: &str) -> Result<Option<ProjectRow>, AppError> {
    let row = sqlx::query_as::<_, ProjectRow>(
        "SELECT id, name, description, owner_id, version, node_count, edge_count,
                data_cipher, data_iv, data_tag, created_at, updated_at, updated_by, status
         FROM projects WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 覆盖拓扑（带版本号条件，返回受影响行数用于冲突判断）
pub async fn update_data(
    conn: &mut SqliteConnection,
    id: &str,
    cipher: &[u8],
    iv: &[u8],
    tag: &[u8],
    expected_version: i64,
    node_count: i64,
    edge_count: i64,
    updated_at: i64,
    updated_by: &str,
) -> Result<u64, AppError> {
    let result = sqlx::query(
        "UPDATE projects
         SET data_cipher = ?, data_iv = ?, data_tag = ?, version = version + 1,
             node_count = ?, edge_count = ?, updated_at = ?, updated_by = ?
         WHERE id = ? AND version = ?",
    )
    .bind(cipher)
    .bind(iv)
    .bind(tag)
    .bind(node_count)
    .bind(edge_count)
    .bind(updated_at)
    .bind(updated_by)
    .bind(id)
    .bind(expected_version)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

/// 更新项目资料
pub async fn update_meta(
    conn: &mut SqliteConnection,
    id: &str,
    name: Option<&str>,
    description: Option<&str>,
    status: Option<&str>,
    updated_at: i64,
    updated_by: &str,
) -> Result<(), AppError> {
    if let Some(v) = name {
        sqlx::query("UPDATE projects SET name = ?, updated_at = ?, updated_by = ? WHERE id = ?")
            .bind(v)
            .bind(updated_at)
            .bind(updated_by)
            .bind(id)
            .execute(&mut *conn)
            .await?;
    }
    if let Some(v) = description {
        sqlx::query("UPDATE projects SET description = ?, updated_at = ?, updated_by = ? WHERE id = ?")
            .bind(v)
            .bind(updated_at)
            .bind(updated_by)
            .bind(id)
            .execute(&mut *conn)
            .await?;
    }
    if let Some(v) = status {
        sqlx::query("UPDATE projects SET status = ?, updated_at = ?, updated_by = ? WHERE id = ?")
            .bind(v)
            .bind(updated_at)
            .bind(updated_by)
            .bind(id)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

/// 删除项目
pub async fn delete(conn: &mut SqliteConnection, id: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM projects WHERE id = ?")
        .bind(id)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// 当前用户可见的项目列表（管理员可见全部）
pub async fn list_visible(
    pool: &SqlitePool,
    user_id: &str,
    is_admin: bool,
    keyword: &str,
) -> Result<Vec<ProjectSummary>, AppError> {
    let has_keyword = if keyword.is_empty() { 0 } else { 1 };
    let pattern = if keyword.is_empty() {
        String::new()
    } else {
        format!("%{keyword}%")
    };

    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            i64,
            i64,
            i64,
            String,
            i64,
            i64,
            String,
            String,
        ),
    >(
        "SELECT p.id, p.name, p.description, p.owner_id, p.version, p.node_count, p.edge_count,
                CASE WHEN p.owner_id = ? THEN 'owner'
                     ELSE COALESCE(m.permission, 'viewer') END AS permission,
                p.created_at, p.updated_at, p.updated_by, p.status
         FROM projects p
         LEFT JOIN project_members m ON m.project_id = p.id AND m.user_id = ?
         WHERE (? = 1 OR p.owner_id = ? OR m.user_id IS NOT NULL)
           AND (? = 0 OR p.name LIKE ? ESCAPE '\\' OR p.description LIKE ? ESCAPE '\\')
         ORDER BY p.updated_at DESC
         LIMIT 300",
    )
    .bind(user_id)
    .bind(user_id)
    .bind(if is_admin { 1 } else { 0 })
    .bind(user_id)
    .bind(has_keyword)
    .bind(&pattern)
    .bind(&pattern)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                name,
                description,
                owner_id,
                version,
                node_count,
                edge_count,
                permission,
                created_at,
                updated_at,
                updated_by,
                status,
            )| ProjectSummary {
                id,
                name,
                description,
                owner_id,
                version,
                node_count,
                edge_count,
                permission,
                created_at,
                updated_at,
                updated_by,
                status,
            },
        )
        .collect())
}

/// 查询某人在项目里的权限
pub async fn permission_of(
    pool: &SqlitePool,
    project_id: &str,
    user_id: &str,
) -> Result<Option<String>, AppError> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT permission FROM project_members WHERE project_id = ? AND user_id = ?",
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(p,)| p))
}

/// 成员列表
pub async fn members(pool: &SqlitePool, project_id: &str) -> Result<Vec<MemberEntry>, AppError> {
    let rows = sqlx::query_as::<_, MemberEntry>(
        "SELECT m.user_id, u.username, u.display_name, m.permission, m.granted_at
         FROM project_members m
         JOIN users u ON u.id = m.user_id
         WHERE m.project_id = ?
         ORDER BY CASE m.permission WHEN 'owner' THEN 0 WHEN 'editor' THEN 1 ELSE 2 END,
                  m.granted_at ASC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 新增/更新授权
pub async fn upsert_member(
    conn: &mut SqliteConnection,
    project_id: &str,
    user_id: &str,
    permission: &str,
    granted_by: &str,
    granted_at: i64,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO project_members (project_id, user_id, permission, granted_by, granted_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(project_id, user_id) DO UPDATE SET permission = excluded.permission,
             granted_by = excluded.granted_by, granted_at = excluded.granted_at",
    )
    .bind(project_id)
    .bind(user_id)
    .bind(permission)
    .bind(granted_by)
    .bind(granted_at)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// 撤销授权
pub async fn remove_member(
    conn: &mut SqliteConnection,
    project_id: &str,
    user_id: &str,
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM project_members WHERE project_id = ? AND user_id = ?")
        .bind(project_id)
        .bind(user_id)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// 统计 owner 数量（防止撤销最后一个 owner）
pub async fn count_owners(conn: &mut SqliteConnection, project_id: &str) -> Result<i64, AppError> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM project_members WHERE project_id = ? AND permission = 'owner'",
    )
    .bind(project_id)
    .fetch_one(&mut *conn)
    .await?;
    Ok(row.0)
}

/// 写入在线心跳
pub async fn presence_ping(
    pool: &SqlitePool,
    project_id: &str,
    user_id: &str,
    user_name: &str,
    x: f64,
    y: f64,
    at: i64,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO presence (project_id, user_id, user_name, cursor_x, cursor_y, seen_at)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(project_id, user_id) DO UPDATE SET user_name = excluded.user_name,
             cursor_x = excluded.cursor_x, cursor_y = excluded.cursor_y, seen_at = excluded.seen_at",
    )
    .bind(project_id)
    .bind(user_id)
    .bind(user_name)
    .bind(x)
    .bind(y)
    .bind(at)
    .execute(pool)
    .await?;
    Ok(())
}

/// 在线成员（时间段内有心跳）
pub async fn presence_list(
    pool: &SqlitePool,
    project_id: &str,
    since: i64,
) -> Result<Vec<PresenceEntry>, AppError> {
    let rows = sqlx::query_as::<_, PresenceEntry>(
        "SELECT user_id, user_name, cursor_x, cursor_y, seen_at
         FROM presence WHERE project_id = ? AND seen_at >= ?
         ORDER BY seen_at DESC",
    )
    .bind(project_id)
    .bind(since)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 清理过期心跳
pub async fn presence_cleanup(pool: &SqlitePool, before: i64) -> Result<(), AppError> {
    sqlx::query("DELETE FROM presence WHERE seen_at < ?")
        .bind(before)
        .execute(pool)
        .await?;
    Ok(())
}
/// 把某个账号从所有项目里移除（删号前清理成员关系）
pub async fn remove_member_everywhere(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: &str,
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM project_members WHERE user_id = ?")
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}