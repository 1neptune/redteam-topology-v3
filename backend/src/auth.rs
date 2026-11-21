//! JWT 鉴权：签发、解析，以及可作为 handler 参数的用户提取器。

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::state::AppState;

/// JWT 载荷；`tv` 与 users.token_version 比对，实现改密即失效。
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub role: String,
    pub tv: i64,
    pub exp: i64,
    pub iat: i64,
}

/// 已认证用户（handler 通过它拿到身份，无需再查库）
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub role: String,
}

impl AuthUser {
    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }
}

/// 签发令牌
pub fn issue_token(
    secret: &str,
    ttl_secs: i64,
    user_id: &str,
    username: &str,
    role: &str,
    token_version: i64,
) -> Result<(String, i64), AppError> {
    let now = crate::service::now_ms() / 1000;
    let exp = now + ttl_secs;
    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        role: role.to_string(),
        tv: token_version,
        exp,
        iat: now,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("签发令牌失败: {e}")))?;
    Ok((token, exp * 1000))
}

/// 从 Authorization 头解析 Bearer 令牌
fn bearer(parts: &Parts) -> Option<String> {
    parts
        .headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let token = bearer(parts)
            .ok_or_else(|| AppError::Unauthorized("请先登录".to_string()))?;

        let data = decode::<Claims>(
            &token,
            &DecodingKey::from_secret(state.cfg.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|_| AppError::Unauthorized("登录状态已失效，请重新登录".to_string()))?;

        // 每次请求校验账号状态与令牌版本，保证停用/改密立即生效
        let row: Option<(i64, i64, String, String)> = sqlx::query_as(
            "SELECT token_version, disabled, display_name, role FROM users WHERE id = ?",
        )
        .bind(&data.claims.sub)
        .fetch_optional(&state.pool)
        .await?;

        let (token_version, disabled, display_name, role) =
            row.ok_or_else(|| AppError::Unauthorized("账号不存在".to_string()))?;

        if disabled != 0 {
            return Err(AppError::Unauthorized("账号已被停用".to_string()));
        }
        if token_version != data.claims.tv {
            return Err(AppError::Unauthorized("口令已变更，请重新登录".to_string()));
        }

        Ok(AuthUser {
            id: data.claims.sub,
            username: data.claims.username,
            display_name,
            role,
        })
    }
}