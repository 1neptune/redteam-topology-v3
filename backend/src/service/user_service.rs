//! 用户管理业务（管理员操作）。

use sqlx::SqlitePool;

use crate::auth::AuthUser;
use crate::db::{crypto, new_id, user_db};
use crate::error::AppError;
use crate::model::user::{CreateUserRequest, UpdateUserRequest};
use crate::model::{PublicUser, User};
use crate::service::{now_ms, tidy, valid_password, valid_role, valid_username};

/// 用户列表（管理员）
pub async fn list(pool: &SqlitePool, actor: &AuthUser) -> Result<Vec<PublicUser>, AppError> {
    if !actor.is_admin() {
        return Err(AppError::Forbidden("仅管理员可查看账号列表".to_string()));
    }
    user_db::list(pool).await
}

/// 建号（管理员）
pub async fn create(
    pool: &SqlitePool,
    actor: &AuthUser,
    req: CreateUserRequest,
) -> Result<PublicUser, AppError> {
    if !actor.is_admin() {
        return Err(AppError::Forbidden("仅管理员可建号".to_string()));
    }
    let username = tidy(&req.username, 32);
    if !valid_username(&username) {
        return Err(AppError::BadRequest(
            "用户名需为 3-32 位字母、数字、下划线或连字符".to_string(),
        ));
    }
    if !valid_password(&req.password) {
        return Err(AppError::BadRequest(crate::service::PASSWORD_RULE.to_string()));
    }
    if user_db::find_by_username(pool, &username).await?.is_some() {
        return Err(AppError::BadRequest("该用户名已被占用".to_string()));
    }

    let role = if valid_role(&req.role) { req.role.clone() } else { "member".to_string() };
    let display_name = {
        let name = tidy(&req.display_name, 32);
        if name.is_empty() {
            username.clone()
        } else {
            name
        }
    };

    let user = User {
        id: new_id("u_"),
        username,
        display_name,
        role,
        password_hash: crypto::hash_password(&req.password)?,
        token_version: 1,
        disabled: 0,
        created_at: now_ms(),
        last_login_at: None,
    };
    user_db::insert(pool, &user).await?;

    // 建号即可直接授权：创建人须能管理该项目
    let project_id = req.project_id.trim().to_string();
    if !project_id.is_empty() {
        let permission = if crate::service::valid_permission(&req.permission) {
            req.permission.clone()
        } else {
            "editor".to_string()
        };
        let (_row, actor_permission) = crate::service::project_service::resolve(pool, actor, &project_id).await?;
        if actor_permission != "owner" {
            return Err(AppError::Forbidden("只有项目所有者可以授权".to_string()));
        }
        let mut tx = pool.begin().await?;
        crate::db::project_db::upsert_member(&mut tx, &project_id, &user.id, &permission, &actor.id, now_ms())
            .await?;
        crate::service::audit_service::record_event(
            &mut tx,
            Some(&project_id),
            actor,
            "member.grant",
            "member",
            &user.id,
            &user.display_name,
            &format!("建号时授权「{}」为{}", user.display_name, permission),
        )
        .await?;
        tx.commit().await?;
    }

    Ok(PublicUser::from(&user))
}

/// 更新账号：角色、启停、显示名、重置口令
pub async fn update(
    pool: &SqlitePool,
    actor: &AuthUser,
    target_id: &str,
    req: UpdateUserRequest,
) -> Result<PublicUser, AppError> {
    if !actor.is_admin() {
        return Err(AppError::Forbidden("仅管理员可修改账号".to_string()));
    }
    let target = user_db::find_by_id(pool, target_id)
        .await?
        .ok_or_else(|| AppError::NotFound("账号不存在".to_string()))?;

    // 防止管理员把自己降级或停用后失去管理能力
    if target.id == actor.id {
        if let Some(role) = req.role.as_deref() {
            if role != "admin" {
                return Err(AppError::BadRequest("不能降低自己的管理员角色".to_string()));
            }
        }
        if req.disabled == Some(true) {
            return Err(AppError::BadRequest("不能停用自己的账号".to_string()));
        }
    }

    let display_name = req.display_name.as_deref().map(|v| tidy(v, 32));
    let role = req.role.as_deref().filter(|v| valid_role(v));
    if req.role.is_some() && role.is_none() {
        return Err(AppError::BadRequest("角色只能是 admin 或 member".to_string()));
    }
    let disabled = req.disabled.map(|v| if v { 1 } else { 0 });

    let password_hash = match req.password.as_deref() {
        Some(p) => {
            if !valid_password(p) {
                return Err(AppError::BadRequest(crate::service::PASSWORD_RULE.to_string()));
            }
            Some(crypto::hash_password(p)?)
        }
        None => None,
    };

    // 改口令或停用时自增 token_version，让在线会话立即失效
    let bump = password_hash.is_some() || disabled == Some(1);

    user_db::update(
        pool,
        target_id,
        display_name.as_deref(),
        role,
        disabled,
        password_hash.as_deref(),
        bump,
    )
    .await?;

    let updated = user_db::find_by_id(pool, target_id)
        .await?
        .ok_or_else(|| AppError::Internal("账号读取失败".to_string()))?;
    Ok(PublicUser::from(&updated))
}

/// 成员名录（普通用户也可读，用于授权选择）
pub async fn directory(pool: &SqlitePool) -> Result<Vec<crate::model::user::DirectoryEntry>, AppError> {
    user_db::directory(pool).await
}
/// 删除账号（管理员）
///
/// 保住两条底线：不能删自己，最后一个管理员不能删。
pub async fn delete(pool: &SqlitePool, actor: &AuthUser, target_id: &str) -> Result<(), AppError> {
    if !actor.is_admin() {
        return Err(AppError::Forbidden("仅管理员可删除账号".to_string()));
    }
    if actor.id == target_id {
        return Err(AppError::BadRequest("不能删除当前登录的账号".to_string()));
    }
    let target = crate::db::user_db::find_by_id(pool, target_id)
        .await?
        .ok_or_else(|| AppError::NotFound("账号不存在".to_string()))?;

    if target.role == "admin" {
        let admins = crate::db::user_db::list(pool)
            .await?
            .iter()
            .filter(|u| u.role == "admin" && !u.disabled)
            .count();
        if admins <= 1 {
            return Err(AppError::BadRequest("至少要保留一个管理员".to_string()));
        }
    }

    let mut tx = pool.begin().await?;
    crate::db::project_db::remove_member_everywhere(&mut tx, target_id).await?;
    crate::db::user_db::delete(&mut tx, target_id).await?;
    crate::service::audit_service::record_event(
        &mut tx,
        None,
        actor,
        "user.delete",
        "user",
        target_id,
        &target.display_name,
        &format!("删除账号「{}」", target.display_name),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}