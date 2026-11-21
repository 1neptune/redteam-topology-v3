//! 项目业务：权限判定、CRUD、保存与审计、导入导出、成员授权、在线状态。

use serde_json::json;
use sqlx::SqlitePool;

use crate::auth::AuthUser;
use crate::config::Config;
use crate::db::{crypto, new_id, project_db};
use crate::error::AppError;
use crate::model::project::{
    CreateProjectRequest, ExportBundle, ExportProject, GrantRequest, ImportRequest, ProjectDetail,
    ProjectRow, ProjectSummary, SaveProjectRequest, SaveResult, UpdateProjectRequest,
};
use crate::model::{Graph, MemberEntry, PresenceEntry};
use crate::service::{audit_service, graph_service, now_ms, tidy};

const MAX_PROJECTS_PER_USER: i64 = 200;

/// 权限等级：数值越大权限越高
fn level(permission: &str) -> u8 {
    match permission {
        "owner" => 3,
        "editor" => 2,
        "viewer" => 1,
        _ => 0,
    }
}

/// 解析当前用户在项目上的有效权限；无权访问统一返回 404，避免探测项目是否存在
pub async fn resolve(
    pool: &SqlitePool,
    actor: &AuthUser,
    project_id: &str,
) -> Result<(ProjectRow, String), AppError> {
    let row = project_db::find(pool, project_id)
        .await?
        .ok_or_else(|| AppError::NotFound("项目不存在或无权访问".to_string()))?;

    if actor.is_admin() {
        return Ok((row, "owner".to_string()));
    }
    if row.owner_id == actor.id {
        return Ok((row, "owner".to_string()));
    }
    match project_db::permission_of(pool, project_id, &actor.id).await? {
        Some(permission) => Ok((row, permission)),
        None => Err(AppError::NotFound("项目不存在或无权访问".to_string())),
    }
}

/// 校验最低权限
fn ensure(permission: &str, need: &str) -> Result<(), AppError> {
    if level(permission) >= level(need) {
        Ok(())
    } else {
        Err(AppError::Forbidden("没有该项目的操作权限".to_string()))
    }
}

/// 解密项目拓扑
fn decrypt(cfg: &Config, row: &ProjectRow) -> Result<Graph, AppError> {
    let bytes = crypto::open(&cfg.master_key, &row.data_cipher, &row.data_iv, &row.data_tag)?;
    serde_json::from_slice::<Graph>(&bytes)
        .map_err(|e| AppError::Internal(format!("拓扑解析失败: {e}")))
}

/// 行 → 列表项
fn to_summary(row: &ProjectRow, permission: &str) -> ProjectSummary {
    ProjectSummary {
        id: row.id.clone(),
        name: row.name.clone(),
        description: row.description.clone(),
        owner_id: row.owner_id.clone(),
        version: row.version,
        node_count: row.node_count,
        edge_count: row.edge_count,
        permission: permission.to_string(),
        created_at: row.created_at,
        updated_at: row.updated_at,
        updated_by: row.updated_by.clone(),
        status: row.status.clone(),
    }
}

/// 项目列表
pub async fn list(pool: &SqlitePool, actor: &AuthUser, keyword: &str) -> Result<Vec<ProjectSummary>, AppError> {
    let kw = tidy(keyword, 60).replace('%', "").replace('_', "");
    project_db::list_visible(pool, &actor.id, actor.is_admin(), &kw).await
}

/// 新建项目
pub async fn create(
    pool: &SqlitePool,
    cfg: &Config,
    actor: &AuthUser,
    req: CreateProjectRequest,
) -> Result<ProjectSummary, AppError> {
    let name = tidy(&req.name, 80);
    if name.is_empty() {
        return Err(AppError::BadRequest("项目名称不能为空".to_string()));
    }
    let owned = project_db::list_visible(pool, &actor.id, actor.is_admin(), "").await?;
    if owned.iter().filter(|p| p.permission == "owner").count() as i64 >= MAX_PROJECTS_PER_USER {
        return Err(AppError::BadRequest("项目数量已达上限".to_string()));
    }

    let graph = graph_service::normalize(req.data.unwrap_or_default());
    create_with_graph(pool, cfg, actor, &name, &tidy(&req.description, 400), graph).await
}

/// 以给定拓扑创建项目（create / import / duplicate / MCP 复用）
pub async fn create_with_graph(
    pool: &SqlitePool,
    cfg: &Config,
    actor: &AuthUser,
    name: &str,
    description: &str,
    graph: Graph,
) -> Result<ProjectSummary, AppError> {
    let at = now_ms();
    let payload = serde_json::to_vec(&graph).map_err(|e| AppError::Internal(format!("序列化失败: {e}")))?;
    let sealed = crypto::seal(&cfg.master_key, &payload)?;

    let row = ProjectRow {
        id: new_id("p_"),
        name: name.to_string(),
        description: description.to_string(),
        owner_id: actor.id.clone(),
        version: 1,
        node_count: graph.nodes.len() as i64,
        edge_count: graph.edges.len() as i64,
        data_cipher: sealed.cipher,
        data_iv: sealed.iv,
        data_tag: sealed.tag,
        created_at: at,
        updated_at: at,
        updated_by: actor.display_name.clone(),
        status: "active".to_string(),
    };

    let mut tx = pool.begin().await?;
    project_db::insert(&mut tx, &row).await?;
    project_db::upsert_member(&mut tx, &row.id, &actor.id, "owner", &actor.id, at).await?;
    audit_service::record_event(
        &mut tx,
        Some(&row.id),
        actor,
        "project.create",
        "project",
        &row.id,
        &row.name,
        &format!("创建项目「{}」", row.name),
    )
    .await?;
    tx.commit().await?;

    Ok(to_summary(&row, "owner"))
}

/// 项目详情（解密拓扑）
pub async fn detail(
    pool: &SqlitePool,
    cfg: &Config,
    actor: &AuthUser,
    project_id: &str,
) -> Result<ProjectDetail, AppError> {
    let (row, permission) = resolve(pool, actor, project_id).await?;
    let graph = decrypt(cfg, &row)?;
    Ok(ProjectDetail {
        project: to_summary(&row, &permission),
        permission,
        graph,
    })
}

/// 保存拓扑：版本冲突检测 + 变更对比 + 审计
pub async fn save(
    pool: &SqlitePool,
    cfg: &Config,
    actor: &AuthUser,
    project_id: &str,
    req: SaveProjectRequest,
) -> Result<SaveResult, AppError> {
    let (row, permission) = resolve(pool, actor, project_id).await?;
    ensure(&permission, "editor")?;

    if let Some(base) = req.base_version {
        if base != row.version {
            return Err(AppError::Conflict(format!(
                "项目已被其他成员更新（当前版本 {}）",
                row.version
            )));
        }
    }

    let before = decrypt(cfg, &row)?;
    let after = graph_service::normalize(req.data);
    // 保护：原图有节点、这次却是空图，多半是客户端发错了数据。
    // 直接落盘会把整张图抹掉，这里拒绝并要求显式删除。
    if !before.nodes.is_empty() && after.nodes.is_empty() {
        return Err(AppError::BadRequest(
            "保存内容为空，已拒绝写入以免覆盖现有拓扑".to_string(),
        ));
    }

    let changes = graph_service::diff(&before, &after);

    let payload = serde_json::to_vec(&after).map_err(|e| AppError::Internal(format!("序列化失败: {e}")))?;
    let sealed = crypto::seal(&cfg.master_key, &payload)?;
    let at = now_ms();

    let mut tx = pool.begin().await?;
    let affected = project_db::update_data(
        &mut tx,
        project_id,
        &sealed.cipher,
        &sealed.iv,
        &sealed.tag,
        row.version,
        after.nodes.len() as i64,
        after.edges.len() as i64,
        at,
        &actor.display_name,
    )
    .await?;

    // 条件更新未命中说明期间被他人抢先保存
    if affected == 0 {
        tx.rollback().await?;
        return Err(AppError::Conflict("项目已被其他成员更新，请刷新后重试".to_string()));
    }

    audit_service::record_changes(&mut tx, project_id, actor, &changes).await?;
    tx.commit().await?;

    super::project_log::record(
        cfg,
        project_id,
        &row.name,
        &actor.display_name,
        "graph.save",
        &format!(
            "v{} 节点 {} 链路 {} 变更 {}",
            row.version + 1,
            after.nodes.len(),
            after.edges.len(),
            changes.len()
        ),
    );

    Ok(SaveResult {
        version: row.version + 1,
        node_count: after.nodes.len() as i64,
        edge_count: after.edges.len() as i64,
        changes: changes.len(),
        updated_at: at,
    })
}

/// 修改项目资料（owner）
pub async fn update_meta(
    pool: &SqlitePool,
    actor: &AuthUser,
    project_id: &str,
    req: UpdateProjectRequest,
) -> Result<ProjectSummary, AppError> {
    let (row, permission) = resolve(pool, actor, project_id).await?;
    ensure(&permission, "owner")?;

    let name = req.name.as_deref().map(|v| tidy(v, 80));
    if let Some(ref n) = name {
        if n.is_empty() {
            return Err(AppError::BadRequest("项目名称不能为空".to_string()));
        }
    }
    let description = req.description.as_deref().map(|v| tidy(v, 400));
    // 状态只有两个取值，其它一律按「进行中」处理
    let status = req.status.as_deref().map(|v| {
        if v == "done" {
            "done".to_string()
        } else {
            "active".to_string()
        }
    });
    let at = now_ms();

    let mut tx = pool.begin().await?;
    project_db::update_meta(
        &mut tx,
        project_id,
        name.as_deref(),
        description.as_deref(),
        status.as_deref(),
        at,
        &actor.display_name,
    )
    .await?;
    audit_service::record_event(
        &mut tx,
        Some(project_id),
        actor,
        "project.update",
        "project",
        project_id,
        name.as_deref().unwrap_or(&row.name),
        "修改项目资料",
    )
    .await?;
    tx.commit().await?;

    let fresh = project_db::find(pool, project_id)
        .await?
        .ok_or_else(|| AppError::NotFound("项目不存在".to_string()))?;
    Ok(to_summary(&fresh, &permission))
}

/// 删除项目（owner）
pub async fn delete(pool: &SqlitePool, actor: &AuthUser, project_id: &str) -> Result<(), AppError> {
    let (row, permission) = resolve(pool, actor, project_id).await?;
    ensure(&permission, "owner")?;

    let mut tx = pool.begin().await?;
    audit_service::record_event(
        &mut tx,
        None,
        actor,
        "project.delete",
        "project",
        project_id,
        &row.name,
        &format!("删除项目「{}」", row.name),
    )
    .await?;
    project_db::delete(&mut tx, project_id).await?;
    tx.commit().await?;
    Ok(())
}

/// 复制项目
pub async fn duplicate(
    pool: &SqlitePool,
    cfg: &Config,
    actor: &AuthUser,
    project_id: &str,
) -> Result<ProjectSummary, AppError> {
    let (row, _) = resolve(pool, actor, project_id).await?;
    let graph = decrypt(cfg, &row)?;
    let name = format!("{} 副本", row.name);
    create_with_graph(pool, cfg, actor, &name, &row.description, graph).await
}

/// 导出为 JSON 结构
pub async fn export(
    pool: &SqlitePool,
    cfg: &Config,
    actor: &AuthUser,
    project_id: &str,
) -> Result<ExportBundle, AppError> {
    let (row, _) = resolve(pool, actor, project_id).await?;
    let graph = decrypt(cfg, &row)?;
    Ok(ExportBundle {
        format: "redteam-atlas".to_string(),
        schema_version: 3,
        exported_at: now_ms(),
        exported_by: actor.display_name.clone(),
        project: ExportProject {
            name: row.name.clone(),
            description: row.description.clone(),
            data: graph,
        },
    })
}

/// 导入 JSON 为新项目
pub async fn import(
    pool: &SqlitePool,
    cfg: &Config,
    actor: &AuthUser,
    req: ImportRequest,
) -> Result<ProjectSummary, AppError> {
    let name = tidy(&req.project.name, 80);
    let name = if name.is_empty() {
        "导入的项目".to_string()
    } else {
        name
    };
    let graph = graph_service::normalize(req.project.data);
    create_with_graph(pool, cfg, actor, &name, &tidy(&req.project.description, 400), graph).await
}

/// 成员列表
pub async fn members(
    pool: &SqlitePool,
    actor: &AuthUser,
    project_id: &str,
) -> Result<Vec<MemberEntry>, AppError> {
    resolve(pool, actor, project_id).await?;
    project_db::members(pool, project_id).await
}

/// 授权
pub async fn grant(
    pool: &SqlitePool,
    actor: &AuthUser,
    project_id: &str,
    req: GrantRequest,
) -> Result<Vec<MemberEntry>, AppError> {
    let (_row, permission) = resolve(pool, actor, project_id).await?;
    ensure(&permission, "owner")?;

    if !crate::service::valid_permission(&req.permission) {
        return Err(AppError::BadRequest("权限只能是 owner、editor 或 viewer".to_string()));
    }
    let target = crate::db::user_db::find_by_id(pool, &req.user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("目标账号不存在".to_string()))?;
    if target.disabled != 0 {
        return Err(AppError::BadRequest("目标账号已停用".to_string()));
    }

    let at = now_ms();
    let mut tx = pool.begin().await?;
    project_db::upsert_member(&mut tx, project_id, &req.user_id, &req.permission, &actor.id, at).await?;
    audit_service::record_event(
        &mut tx,
        Some(project_id),
        actor,
        "member.grant",
        "member",
        &req.user_id,
        &target.display_name,
        &format!(
            "授权「{}」为{}",
            target.display_name,
            permission_label(&req.permission)
        ),
    )
    .await?;
    tx.commit().await?;

    project_db::members(pool, project_id).await
}

/// 撤销授权
pub async fn revoke(
    pool: &SqlitePool,
    actor: &AuthUser,
    project_id: &str,
    user_id: &str,
) -> Result<Vec<MemberEntry>, AppError> {
    let (_row, permission) = resolve(pool, actor, project_id).await?;
    ensure(&permission, "owner")?;

    let mut tx = pool.begin().await?;
    let owners = project_db::count_owners(&mut tx, project_id).await?;
    if owners <= 1 {
        // 只剩一个 owner 时不允许移除，避免项目无人可管理
        let remaining = project_db::members(pool, project_id).await?;
        let is_owner = remaining
            .iter()
            .any(|m| m.user_id == user_id && m.permission == "owner");
        if is_owner {
            tx.rollback().await?;
            return Err(AppError::BadRequest("项目至少需要保留一位管理者".to_string()));
        }
    }

    let name = crate::db::user_db::find_by_id(pool, user_id)
        .await?
        .map(|u| u.display_name)
        .unwrap_or_else(|| user_id.to_string());

    project_db::remove_member(&mut tx, project_id, user_id).await?;
    audit_service::record_event(
        &mut tx,
        Some(project_id),
        actor,
        "member.revoke",
        "member",
        user_id,
        &name,
        &format!("撤销「{name}」的访问权限"),
    )
    .await?;
    tx.commit().await?;

    project_db::members(pool, project_id).await
}

fn permission_label(permission: &str) -> &'static str {
    match permission {
        "owner" => "管理者",
        "editor" => "可编辑成员",
        _ => "只读成员",
    }
}

/// 记录在线心跳
pub async fn ping(
    pool: &SqlitePool,
    actor: &AuthUser,
    project_id: &str,
    x: f64,
    y: f64,
) -> Result<(), AppError> {
    resolve(pool, actor, project_id).await?;
    let clamp = |v: f64| if v.is_finite() { v.clamp(-100_000.0, 100_000.0) } else { 0.0 };
    project_db::presence_ping(pool, project_id, &actor.id, &actor.display_name, clamp(x), clamp(y), now_ms())
        .await
}

/// 在线成员快照（15 秒内有心跳）
pub async fn presence(
    pool: &SqlitePool,
    actor: &AuthUser,
    project_id: &str,
) -> Result<Vec<PresenceEntry>, AppError> {
    resolve(pool, actor, project_id).await?;
    let since = now_ms() - 15_000;
    project_db::presence_list(pool, project_id, since).await
}

/// 供 MCP 复用的项目摘要（不校验权限，调用方已确认）
pub fn summary_json(summary: &ProjectSummary) -> serde_json::Value {
    json!({
        "id": summary.id,
        "name": summary.name,
        "nodeCount": summary.node_count,
        "edgeCount": summary.edge_count,
        "version": summary.version,
        "permission": summary.permission,
        "updatedAt": summary.updated_at,
        "updatedBy": summary.updated_by,
        "status": summary.status,
    })
}