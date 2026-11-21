//! 用户相关结构体。

use serde::{Deserialize, Serialize};

/// users 表实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub password_hash: String,
    pub token_version: i64,
    pub disabled: i64,
    pub created_at: i64,
    pub last_login_at: Option<i64>,
}

/// 对外用户视图（不含口令哈希）
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PublicUser {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub disabled: bool,
    pub created_at: i64,
    pub last_login_at: Option<i64>,
}

impl From<&User> for PublicUser {
    fn from(u: &User) -> Self {
        Self {
            id: u.id.clone(),
            username: u.username.clone(),
            display_name: u.display_name.clone(),
            role: u.role.clone(),
            disabled: u.disabled != 0,
            created_at: u.created_at,
            last_login_at: u.last_login_at,
        }
    }
}

/// 登录/注册成功后的载荷
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResult {
    pub token: String,
    pub expires_at: i64,
    pub user: PublicUser,
}

/// 成员名录（授权选择器用，只暴露必要字段）
#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryEntry {
    pub id: String,
    pub username: String,
    pub display_name: String,
}

/// 登录请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// 修改口令请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordRequest {
    pub current: String,
    pub next: String,
}

/// 管理员建号请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub role: String,
    /// 建号后直接授权的项目（可空）
    #[serde(default)]
    pub project_id: String,
    /// 授权级别：owner / editor / viewer
    #[serde(default)]
    pub permission: String,
}

/// 管理员更新用户请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserRequest {
    pub display_name: Option<String>,
    pub role: Option<String>,
    pub disabled: Option<bool>,
    pub password: Option<String>,
}