//! 认证业务：登录、改密、初始管理员引导、凭据校验（供 MCP Basic 鉴权）。

use rand::Rng;
use sqlx::SqlitePool;

use crate::auth::{issue_token, AuthUser};
use crate::config::Config;
use crate::db::{crypto, new_id, user_db};
use crate::error::AppError;
use crate::model::user::{ChangePasswordRequest, LoginRequest, LoginResult, User};
use crate::model::PublicUser;
use crate::service::{now_ms, tidy, valid_password, valid_username};

/// 生成随机口令（用于首启管理员）
fn random_password(len: usize) -> String {
    const UPPER: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";
    const LOWER: &[u8] = b"abcdefghijkmnopqrstuvwxyz";
    const DIGIT: &[u8] = b"23456789";
    const ALL: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";

    let mut rng = rand::rng();
    // 前两位固定一大写一小写，保证满足口令策略
    let mut out: Vec<char> = vec![
        UPPER[rng.random_range(0..UPPER.len())] as char,
        LOWER[rng.random_range(0..LOWER.len())] as char,
        DIGIT[rng.random_range(0..DIGIT.len())] as char,
    ];
    while out.len() < len.max(8) {
        out.push(ALL[rng.random_range(0..ALL.len())] as char);
    }
    out.into_iter().collect()
}

/// 登录（系统唯一公开入口）
pub async fn login(pool: &SqlitePool, cfg: &Config, req: LoginRequest) -> Result<LoginResult, AppError> {
    let username = tidy(&req.username, 32);
    let user = user_db::find_by_username(pool, &username)
        .await?
        .ok_or_else(|| AppError::Unauthorized("用户名或口令错误".to_string()))?;

    if !crypto::verify_password(&user.password_hash, &req.password) {
        return Err(AppError::Unauthorized("用户名或口令错误".to_string()));
    }
    if user.disabled != 0 {
        return Err(AppError::Forbidden("账号已被停用，请联系管理员".to_string()));
    }

    user_db::touch_login(pool, &user.id, now_ms()).await?;

    let (token, expires_at) = issue_token(
        &cfg.jwt_secret,
        cfg.jwt_ttl_secs,
        &user.id,
        &user.username,
        &user.role,
        user.token_version,
    )?;

    Ok(LoginResult {
        token,
        expires_at,
        user: PublicUser::from(&user),
    })
}

/// 校验用户名口令并返回身份，用于 MCP 的 HTTP Basic 鉴权（不签发令牌）。
pub async fn verify_credentials(
    pool: &SqlitePool,
    username: &str,
    password: &str,
) -> Result<AuthUser, AppError> {
    let user = user_db::find_by_username(pool, &tidy(username, 32))
        .await?
        .ok_or_else(|| AppError::Unauthorized("用户名或口令错误".to_string()))?;

    if !crypto::verify_password(&user.password_hash, password) {
        return Err(AppError::Unauthorized("用户名或口令错误".to_string()));
    }
    if user.disabled != 0 {
        return Err(AppError::Forbidden("账号已被停用".to_string()));
    }

    Ok(AuthUser {
        id: user.id,
        username: user.username,
        display_name: user.display_name,
        role: user.role,
    })
}

/// 修改口令：成功后旧令牌全部失效，返回新令牌
pub async fn change_password(
    pool: &SqlitePool,
    cfg: &Config,
    user_id: &str,
    req: ChangePasswordRequest,
) -> Result<LoginResult, AppError> {
    if !valid_password(&req.next) {
        return Err(AppError::BadRequest("新口令长度需为 6-128 位".to_string()));
    }
    let user = user_db::find_by_id(pool, user_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("账号不存在".to_string()))?;

    if !crypto::verify_password(&user.password_hash, &req.current) {
        return Err(AppError::BadRequest("当前口令不正确".to_string()));
    }
    if req.current == req.next {
        return Err(AppError::BadRequest("新口令不能与当前口令相同".to_string()));
    }

    let hash = crypto::hash_password(&req.next)?;
    user_db::update(pool, user_id, None, None, None, Some(&hash), true).await?;

    let updated = user_db::find_by_id(pool, user_id)
        .await?
        .ok_or_else(|| AppError::Internal("账号读取失败".to_string()))?;

    let (token, expires_at) = issue_token(
        &cfg.jwt_secret,
        cfg.jwt_ttl_secs,
        &updated.id,
        &updated.username,
        &updated.role,
        updated.token_version,
    )?;

    Ok(LoginResult {
        token,
        expires_at,
        user: PublicUser::from(&updated),
    })
}

/// 首次启动引导：无账号时创建管理员，返回 (用户名, 口令)
pub async fn ensure_admin(pool: &SqlitePool, cfg: &Config) -> Result<Option<(String, String)>, AppError> {
    if user_db::count(pool).await? > 0 {
        return Ok(None);
    }
    let username = tidy(&cfg.bootstrap_admin, 32);
    if !valid_username(&username) {
        return Err(AppError::BadRequest(
            "ATLAS_ADMIN_USER 需为 3-32 位字母、数字、下划线或连字符".to_string(),
        ));
    }

    let password = cfg
        .bootstrap_password
        .clone()
        .unwrap_or_else(|| random_password(14));
    if !valid_password(&password) {
        return Err(AppError::BadRequest(
            format!("ATLAS_ADMIN_PASSWORD 不符合要求：{}", crate::service::PASSWORD_RULE),
        ));
    }

    let user = User {
        id: new_id("u_"),
        username,
        display_name: "管理员".to_string(),
        role: "admin".to_string(),
        password_hash: crypto::hash_password(&password)?,
        token_version: 1,
        disabled: 0,
        created_at: now_ms(),
        last_login_at: None,
    };
    user_db::insert(pool, &user).await?;
    Ok(Some((user.username, password)))
}