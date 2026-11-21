//! users 表的 SQL 操作。

use sqlx::SqlitePool;

use crate::error::AppError;
use crate::model::{DirectoryEntry, PublicUser, User};

/// 统计账号总数
pub async fn count(pool: &SqlitePool) -> Result<i64, AppError> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

/// 按用户名查实体
pub async fn find_by_username(pool: &SqlitePool, username: &str) -> Result<Option<User>, AppError> {
    let row = sqlx::query_as::<_, User>(
        "SELECT id, username, display_name, role, password_hash, token_version, disabled,
                created_at, last_login_at
         FROM users WHERE username = ?",
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 按 ID 查实体
pub async fn find_by_id(pool: &SqlitePool, id: &str) -> Result<Option<User>, AppError> {
    let row = sqlx::query_as::<_, User>(
        "SELECT id, username, display_name, role, password_hash, token_version, disabled,
                created_at, last_login_at
         FROM users WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 插入账号
pub async fn insert(pool: &SqlitePool, user: &User) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO users (id, username, display_name, role, password_hash, token_version,
                            disabled, created_at, last_login_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&user.id)
    .bind(&user.username)
    .bind(&user.display_name)
    .bind(&user.role)
    .bind(&user.password_hash)
    .bind(user.token_version)
    .bind(user.disabled)
    .bind(user.created_at)
    .bind(user.last_login_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// 更新最近登录时间
pub async fn touch_login(pool: &SqlitePool, id: &str, at: i64) -> Result<(), AppError> {
    sqlx::query("UPDATE users SET last_login_at = ? WHERE id = ?")
        .bind(at)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 更新资料（字段按需）
pub async fn update(
    pool: &SqlitePool,
    id: &str,
    display_name: Option<&str>,
    role: Option<&str>,
    disabled: Option<i64>,
    password_hash: Option<&str>,
    bump_token: bool,
) -> Result<(), AppError> {
    if let Some(v) = display_name {
        sqlx::query("UPDATE users SET display_name = ? WHERE id = ?")
            .bind(v)
            .bind(id)
            .execute(pool)
            .await?;
    }
    if let Some(v) = role {
        sqlx::query("UPDATE users SET role = ? WHERE id = ?")
            .bind(v)
            .bind(id)
            .execute(pool)
            .await?;
    }
    if let Some(v) = disabled {
        sqlx::query("UPDATE users SET disabled = ? WHERE id = ?")
            .bind(v)
            .bind(id)
            .execute(pool)
            .await?;
    }
    if let Some(v) = password_hash {
        sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
            .bind(v)
            .bind(id)
            .execute(pool)
            .await?;
    }
    if bump_token {
        sqlx::query("UPDATE users SET token_version = token_version + 1 WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

/// 全部账号（管理员视图）
pub async fn list(pool: &SqlitePool) -> Result<Vec<PublicUser>, AppError> {
    let rows = sqlx::query_as::<_, (String, String, String, String, i64, i64, Option<i64>)>(
        "SELECT id, username, display_name, role, disabled, created_at, last_login_at
         FROM users ORDER BY created_at ASC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, username, display_name, role, disabled, created_at, last_login_at)| {
            PublicUser {
                id,
                username,
                display_name,
                role,
                disabled: disabled != 0,
                created_at,
                last_login_at,
            }
        })
        .collect())
}

/// 名录（仅启用账号）
pub async fn directory(pool: &SqlitePool) -> Result<Vec<DirectoryEntry>, AppError> {
    let rows = sqlx::query_as::<_, DirectoryEntry>(
        "SELECT id, username, display_name FROM users WHERE disabled = 0 ORDER BY display_name ASC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
/// 删除账号
pub async fn delete(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>, id: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}