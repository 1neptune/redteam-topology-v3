//! asset_types 与 uploads 表的 SQL 操作。

use sqlx::SqlitePool;

use crate::error::AppError;
use crate::model::AssetType;

/// 组件类型清单（内置在前）
pub async fn list_types(pool: &SqlitePool) -> Result<Vec<AssetType>, AppError> {
    let rows = sqlx::query_as::<_, AssetType>(
        "SELECT slug, label, color, icon_url, builtin, created_by, created_at, updated_at
         FROM asset_types ORDER BY builtin DESC, created_at ASC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 按 slug 查
pub async fn find_type(pool: &SqlitePool, slug: &str) -> Result<Option<AssetType>, AppError> {
    let row = sqlx::query_as::<_, AssetType>(
        "SELECT slug, label, color, icon_url, builtin, created_by, created_at, updated_at
         FROM asset_types WHERE slug = ?",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 插入自定义组件
pub async fn insert_type(pool: &SqlitePool, item: &AssetType) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO asset_types (slug, label, color, icon_url, builtin, created_by, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&item.slug)
    .bind(&item.label)
    .bind(&item.color)
    .bind(&item.icon_url)
    .bind(item.builtin)
    .bind(&item.created_by)
    .bind(item.created_at)
    .bind(item.updated_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// 更新自定义组件
pub async fn update_type(
    pool: &SqlitePool,
    slug: &str,
    label: &str,
    color: &str,
    icon_url: Option<&str>,
    updated_at: i64,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE asset_types SET label = ?, color = ?, icon_url = ?, updated_at = ? WHERE slug = ?",
    )
    .bind(label)
    .bind(color)
    .bind(icon_url)
    .bind(updated_at)
    .bind(slug)
    .execute(pool)
    .await?;
    Ok(())
}

/// 删除自定义组件
pub async fn delete_type(pool: &SqlitePool, slug: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM asset_types WHERE slug = ? AND builtin = 0")
        .bind(slug)
        .execute(pool)
        .await?;
    Ok(())
}

/// 统计自定义组件数量（用于限制滥用）
pub async fn count_custom_types(pool: &SqlitePool, created_by: &str) -> Result<i64, AppError> {
    let row: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM asset_types WHERE builtin = 0 AND created_by = ?")
            .bind(created_by)
            .fetch_one(pool)
            .await?;
    Ok(row.0)
}

/// 记录上传
pub async fn insert_upload(
    pool: &SqlitePool,
    id: &str,
    filename: &str,
    mime: &str,
    bytes: i64,
    storage: &str,
    created_by: &str,
    created_at: i64,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO uploads (id, filename, mime, bytes, storage, created_by, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(filename)
    .bind(mime)
    .bind(bytes)
    .bind(storage)
    .bind(created_by)
    .bind(created_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// 查询上传元信息
pub async fn find_upload(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<(String, String, String)>, AppError> {
    let row: Option<(String, String, String)> =
        sqlx::query_as("SELECT mime, storage, filename FROM uploads WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    Ok(row)
}

/// 写入内置组件（仅在不存在时插入，保证升级幂等）
pub async fn seed_builtin(pool: &SqlitePool, items: &[AssetType]) -> Result<(), AppError> {
    for item in items {
        sqlx::query(
            "INSERT INTO asset_types (slug, label, color, icon_url, builtin, created_by, created_at, updated_at)
             VALUES (?, ?, ?, NULL, 1, NULL, ?, ?)
             ON CONFLICT(slug) DO NOTHING",
        )
        .bind(&item.slug)
        .bind(&item.label)
        .bind(&item.color)
        .bind(item.created_at)
        .bind(item.updated_at)
        .execute(pool)
        .await?;
    }
    Ok(())
}