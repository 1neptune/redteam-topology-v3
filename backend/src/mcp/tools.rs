//! MCP 工具实现：把 AI Agent 的渗透结果落成攻击路径拓扑。

use std::collections::{HashMap, HashSet};

use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::AuthUser;
use crate::db::new_id;
use crate::error::AppError;
use crate::model::project::SaveProjectRequest;
use crate::model::{EdgeData, Graph, GraphEdge, GraphNode, NodeData, Position};
use crate::service::project_service;
use crate::state::AppState;

/// 工具声明（供 tools/list 返回）
pub fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "list_projects",
            "description": "列出当前令牌可见的攻击路径项目（含节点数、链路数、版本）",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
        }),
        json!({
            "name": "get_project",
            "description": "读取指定项目的完整拓扑（节点与链路）",
            "inputSchema": {
                "type": "object",
                "properties": { "project_id": { "type": "string" } },
                "required": ["project_id"]
            }
        }),
        json!({
            "name": "create_project",
            "description": "新建一个空项目",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "description": { "type": "string" }
                },
                "required": ["name"]
            }
        }),
        json!({
            "name": "ingest_attack_path",
            "description": "把渗透测试结果（节点 + 链路）一次性画成攻击路径；项目不存在时按 project_name 自动创建，自动分层布局并记录审计",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": { "type": "string" },
                    "project_name": { "type": "string" },
                    "nodes": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "label": { "type": "string" },
                                "kind": { "type": "string" },
                                "ip": { "type": "string" },
                                "port": { "type": "string" },
                                "os": { "type": "string" },
                                "status": { "type": "string", "enum": ["unknown", "recon", "vuln", "owned", "blocked"] },
                                "notes": { "type": "string" }
                            },
                            "required": ["label"]
                        }
                    },
                    "edges": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "from": { "type": "string" },
                                "to": { "type": "string" },
                                "label": { "type": "string" },
                                "technique": { "type": "string" },
                                "status": { "type": "string", "enum": ["planned", "active", "success", "failed"] }
                            },
                            "required": ["from", "to"]
                        }
                    }
                }
            }
        }),
        json!({
            "name": "add_nodes",
            "description": "向已有项目追加节点",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": { "type": "string" },
                    "nodes": { "type": "array", "items": { "type": "object" } }
                },
                "required": ["project_id", "nodes"]
            }
        }),
        json!({
            "name": "add_edges",
            "description": "向已有项目追加链路（source/target 用节点 id 或名称）",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": { "type": "string" },
                    "edges": { "type": "array", "items": { "type": "object" } }
                },
                "required": ["project_id", "edges"]
            }
        }),
        json!({
            "name": "update_node",
            "description": "更新单个节点的字段（名称、IP、状态、备注等）",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": { "type": "string" },
                    "node_id": { "type": "string" },
                    "patch": { "type": "object" }
                },
                "required": ["project_id", "node_id", "patch"]
            }
        }),
        json!({
            "name": "set_vulnerability",
            "description": "写入节点的一条渗透测试漏洞（名称、说明、风险等级、渗透详情、处置建议）；不传 index 时追加新的一条，传 index 时覆盖第 index 条（从 0 开始）",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": { "type": "string" },
                    "node_id": { "type": "string", "description": "节点 id 或 label" },
                    "index": { "type": "integer", "description": "第几条漏洞（从 0 开始）；省略则追加新的一条" },
                    "name": { "type": "string", "description": "漏洞名称，如「任意文件上传漏洞」" },
                    "description": { "type": "string", "description": "漏洞说明：影响范围与原理" },
                    "risk": {
                        "type": "string",
                        "enum": ["critical", "high", "medium", "low", "info"],
                        "description": "风险等级"
                    },
                    "detail": { "type": "string", "description": "渗透测试详情：复现步骤、请求报文、验证结果" },
                    "remediation": { "type": "string", "description": "处置建议" }
                },
                "required": ["project_id", "node_id"]
            }
        }),
        json!({
            "name": "remove_vulnerability",
            "description": "删除节点的第 index 条漏洞记录（从 0 开始）",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": { "type": "string" },
                    "node_id": { "type": "string" },
                    "index": { "type": "integer" }
                },
                "required": ["project_id", "node_id", "index"]
            }
        }),
        json!({
            "name": "search_graph",
            "description": "在项目内按关键字搜索节点",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": { "type": "string" },
                    "keyword": { "type": "string" }
                },
                "required": ["project_id", "keyword"]
            }
        }),
        json!({
            "name": "list_asset_types",
            "description": "列出可用的资产组件类型（内置 + 自定义）",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
        }),
        json!({
            "name": "update_project",
            "description": "改项目名或描述（需 owner）",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": { "type": "string" },
                    "name": { "type": "string" },
                    "description": { "type": "string" }
                },
                "required": ["project_id"]
            }
        }),
        json!({
            "name": "delete_project",
            "description": "删除项目及其拓扑（需 owner）",
            "inputSchema": {
                "type": "object",
                "properties": { "project_id": { "type": "string" } },
                "required": ["project_id"]
            }
        }),
        json!({
            "name": "delete_node",
            "description": "删除节点，连带删除它的所有链路；node_id 可填 label 或内部 id",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": { "type": "string" },
                    "node_id": { "type": "string" }
                },
                "required": ["project_id", "node_id"]
            }
        }),
        json!({
            "name": "delete_edge",
            "description": "按链路 id 删除一条链路",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": { "type": "string" },
                    "edge_id": { "type": "string" }
                },
                "required": ["project_id", "edge_id"]
            }
        }),
        json!({
            "name": "create_asset_type",
            "description": "新建自定义资产组件类型",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "label": { "type": "string", "description": "显示名，2-12 字" },
                    "color": { "type": "string", "description": "十六进制色值，如 #c96f7a" },
                    "icon_url": { "type": "string", "description": "可选，builtin:target 或 /api/uploads/xxx" }
                },
                "required": ["label"]
            }
        }),
        json!({
            "name": "add_member",
            "description": "把已有账号授权到项目（需项目 owner）",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": { "type": "string" },
                    "username": { "type": "string" },
                    "permission": { "type": "string", "enum": ["owner", "editor", "viewer"] }
                },
                "required": ["project_id", "username"]
            }
        }),
        json!({
            "name": "list_users",
            "description": "列出全部账号（需系统管理员）",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
        }),
        json!({
            "name": "create_user",
            "description": "新建账号（需系统管理员）。口令至少 8 位且含大写与小写；可同时授权到项目",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "username": { "type": "string", "description": "3-32 位字母、数字、下划线或连字符" },
                    "password": { "type": "string", "description": "至少 8 位，含大小写字母" },
                    "display_name": { "type": "string" },
                    "role": { "type": "string", "enum": ["admin", "member"] },
                    "project_id": { "type": "string", "description": "可选，建号后直接授权" },
                    "permission": { "type": "string", "enum": ["owner", "editor", "viewer"] }
                },
                "required": ["username", "password"]
            }
        }),
    ]
}

#[derive(Debug, Deserialize)]
struct IngestNode {
    #[serde(default)]
    id: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    ip: String,
    #[serde(default)]
    port: String,
    #[serde(default)]
    os: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    notes: String,
}

#[derive(Debug, Deserialize)]
struct IngestEdge {
    from: String,
    to: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    status: String,
}

fn arg_str(args: &Value, key: &str) -> String {
    args.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// 工具调度
pub async fn call(
    state: &AppState,
    user: &AuthUser,
    name: &str,
    args: &Value,
) -> Result<Value, AppError> {
    match name {
        "list_projects" => {
            let list = project_service::list(&state.pool, user, "").await?;
            Ok(json!(list
                .iter()
                .map(project_service::summary_json)
                .collect::<Vec<_>>()))
        }
        "get_project" => {
            let id = arg_str(args, "project_id");
            if id.is_empty() {
                return Err(AppError::BadRequest("project_id 不能为空".to_string()));
            }
            let detail = project_service::detail(&state.pool, &state.cfg, user, &id).await?;
            Ok(json!({
                "project": project_service::summary_json(&detail.project),
                "nodes": detail.graph.nodes,
                "edges": detail.graph.edges,
            }))
        }
        "create_project" => {
            let name = arg_str(args, "name");
            if name.is_empty() {
                return Err(AppError::BadRequest("name 不能为空".to_string()));
            }
            let created = project_service::create(
                &state.pool,
                &state.cfg,
                user,
                crate::model::project::CreateProjectRequest {
                    name,
                    description: arg_str(args, "description"),
                    data: None,
                    status: None,
                },
            )
            .await?;
            Ok(project_service::summary_json(&created))
        }
        "ingest_attack_path" => ingest(state, user, args).await,
        "add_nodes" => {
            let id = arg_str(args, "project_id");
            let nodes: Vec<IngestNode> = serde_json::from_value(
                args.get("nodes").cloned().unwrap_or_else(|| json!([])),
            )
            .map_err(|e| AppError::BadRequest(format!("nodes 格式错误：{e}")))?;
            append(state, user, &id, nodes, Vec::new()).await
        }
        "add_edges" => {
            let id = arg_str(args, "project_id");
            let edges: Vec<IngestEdge> = serde_json::from_value(
                args.get("edges").cloned().unwrap_or_else(|| json!([])),
            )
            .map_err(|e| AppError::BadRequest(format!("edges 格式错误：{e}")))?;
            append(state, user, &id, Vec::new(), edges).await
        }
        "update_node" => {
            let project_id = arg_str(args, "project_id");
            let node_id = arg_str(args, "node_id");
            let patch = args.get("patch").cloned().unwrap_or_else(|| json!({}));
            update_node(state, user, &project_id, &node_id, &patch).await
        }
        "set_vulnerability" => {
            let project_id = arg_str(args, "project_id");
            let node_id = arg_str(args, "node_id");
            set_vulnerability(state, user, &project_id, &node_id, args).await
        }
        "remove_vulnerability" => {
            let project_id = arg_str(args, "project_id");
            let node_id = arg_str(args, "node_id");
            remove_vulnerability(state, user, &project_id, &node_id, args).await
        }
        "search_graph" => {
            let project_id = arg_str(args, "project_id");
            let keyword = arg_str(args, "keyword").to_lowercase();
            let detail = project_service::detail(&state.pool, &state.cfg, user, &project_id).await?;
            let hits: Vec<Value> = detail
                .graph
                .nodes
                .iter()
                .filter(|n| {
                    keyword.is_empty()
                        || n.data.label.to_lowercase().contains(&keyword)
                        || n.data.ip.to_lowercase().contains(&keyword)
                        || n.data.os.to_lowercase().contains(&keyword)
                        || n.data.notes.to_lowercase().contains(&keyword)
                })
                .map(|n| {
                    json!({
                        "id": n.id,
                        "label": n.data.label,
                        "kind": n.data.kind,
                        "ip": n.data.ip,
                        "status": n.data.status
                    })
                })
                .collect();
            Ok(json!({ "count": hits.len(), "nodes": hits }))
        }
        "list_asset_types" => {
            let list = crate::service::asset_service::list(&state.pool).await?;
            Ok(json!(list
                .iter()
                .map(|t| json!({ "slug": t.slug, "label": t.label, "builtin": t.builtin }))
                .collect::<Vec<_>>()))
        }
        "update_project" => {
            let id = arg_str(args, "project_id");
            let name = arg_str(args, "name");
            let updated = project_service::update_meta(
                &state.pool,
                user,
                &id,
                crate::model::project::UpdateProjectRequest {
                    name: if name.is_empty() { None } else { Some(name) },
                    description: args.get("description").map(|v| v.as_str().unwrap_or_default().to_string()),
                    status: args.get("status").map(|v| v.as_str().unwrap_or_default().to_string()),
                },
            )
            .await?;
            Ok(project_service::summary_json(&updated))
        }
        "delete_project" => {
            let id = arg_str(args, "project_id");
            if id.is_empty() {
                return Err(AppError::BadRequest("project_id 不能为空".to_string()));
            }
            project_service::delete(&state.pool, user, &id).await?;
            Ok(json!({ "deleted": id }))
        }
        "delete_node" => {
            let project_id = arg_str(args, "project_id");
            let node_id = arg_str(args, "node_id");
            if node_id.is_empty() {
                return Err(AppError::BadRequest("node_id 不能为空".to_string()));
            }
            let (pid, mut graph) = locate_by_id(state, user, &project_id).await?;
            let before = graph.nodes.len();
            let internal = resolve_node_ref(&graph, &node_id)?;
            graph.nodes.retain(|n| n.id != internal);
            graph.edges.retain(|e| e.source != internal && e.target != internal);
            let removed = before - graph.nodes.len();
            if removed == 0 {
                return Err(AppError::NotFound("节点不存在".to_string()));
            }
            save_graph(state, user, &pid, &graph).await
        }
        "delete_edge" => {
            let project_id = arg_str(args, "project_id");
            let edge_id = arg_str(args, "edge_id");
            if edge_id.is_empty() {
                return Err(AppError::BadRequest("edge_id 不能为空".to_string()));
            }
            let (pid, mut graph) = locate_by_id(state, user, &project_id).await?;
            let before = graph.edges.len();
            graph.edges.retain(|e| e.id != edge_id);
            if graph.edges.len() == before {
                return Err(AppError::NotFound("链路不存在".to_string()));
            }
            save_graph(state, user, &pid, &graph).await
        }
        "create_asset_type" => {
            let created = crate::service::asset_service::create(
                &state.pool,
                user,
                crate::model::misc::AssetTypeRequest {
                    label: arg_str(args, "label"),
                    color: arg_str(args, "color"),
                    icon_url: {
                        let value = arg_str(args, "icon_url");
                        if value.is_empty() { None } else { Some(value) }
                    },
                },
            )
            .await?;
            Ok(json!({ "slug": created.slug, "label": created.label, "iconUrl": created.icon_url }))
        }
        "add_member" => {
            let project_id = arg_str(args, "project_id");
            let username = arg_str(args, "username");
            if project_id.is_empty() || username.is_empty() {
                return Err(AppError::BadRequest("project_id 与 username 都不能为空".to_string()));
            }
            let target = crate::db::user_db::find_by_username(&state.pool, &username)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("账号 {username} 不存在")))?;
            let permission = {
                let value = arg_str(args, "permission");
                if value.is_empty() { "editor".to_string() } else { value }
            };
            let members = project_service::grant(
                &state.pool,
                user,
                &project_id,
                crate::model::project::GrantRequest { user_id: target.id, permission },
            )
            .await?;
            Ok(json!(members
                .iter()
                .map(|m| json!({ "userId": m.user_id, "username": m.username, "permission": m.permission }))
                .collect::<Vec<_>>()))
        }
        "list_users" => {
            let list = crate::service::user_service::list(&state.pool, user).await?;
            Ok(json!(list
                .iter()
                .map(|u| json!({
                    "id": u.id,
                    "username": u.username,
                    "displayName": u.display_name,
                    "role": u.role,
                    "disabled": u.disabled
                }))
                .collect::<Vec<_>>()))
        }
        "create_user" => {
            let created = crate::service::user_service::create(
                &state.pool,
                user,
                crate::model::user::CreateUserRequest {
                    username: arg_str(args, "username"),
                    password: arg_str(args, "password"),
                    display_name: arg_str(args, "display_name"),
                    role: arg_str(args, "role"),
                    project_id: arg_str(args, "project_id"),
                    permission: arg_str(args, "permission"),
                },
            )
            .await?;
            Ok(json!({
                "id": created.id,
                "username": created.username,
                "displayName": created.display_name,
                "role": created.role
            }))
        }
        other => Err(AppError::BadRequest(format!("未知工具：{other}"))),
    }
}

/// 按 project_id 定位并返回可写图（不含名称回退，供删除类工具用）
async fn locate_by_id(
    state: &AppState,
    user: &AuthUser,
    project_id: &str,
) -> Result<(String, Graph), AppError> {
    if project_id.is_empty() {
        return Err(AppError::BadRequest("project_id 不能为空".to_string()));
    }
    let detail = project_service::detail(&state.pool, &state.cfg, user, project_id).await?;
    Ok((detail.project.id, detail.graph))
}

/// 把 label 或内部 id 解析成内部节点 id
fn resolve_node_ref(graph: &Graph, reference: &str) -> Result<String, AppError> {
    if graph.nodes.iter().any(|n| n.id == reference) {
        return Ok(reference.to_string());
    }
    graph
        .nodes
        .iter()
        .find(|n| n.data.label == reference)
        .map(|n| n.id.clone())
        .ok_or_else(|| AppError::NotFound(format!("节点 {reference} 不存在")))
}

/// 保存整图并返回版本与统计
async fn save_graph(
    state: &AppState,
    user: &AuthUser,
    project_id: &str,
    graph: &Graph,
) -> Result<Value, AppError> {
    let result = project_service::save(
        &state.pool,
        &state.cfg,
        user,
        project_id,
        crate::model::project::SaveProjectRequest {
            base_version: None,
            data: graph.clone(),
        },
    )
    .await?;
    Ok(json!({
        "version": result.version,
        "nodes": graph.nodes.len(),
        "edges": graph.edges.len(),
        "editor_path": format!("/#/p/{project_id}")
    }))
}

/// 解析或创建目标项目，返回 (project_id, version, graph)
async fn locate(
    state: &AppState,
    user: &AuthUser,
    args: &Value,
) -> Result<(String, i64, Graph), AppError> {
    let project_id = arg_str(args, "project_id");
    let project_name = arg_str(args, "project_name");

    if !project_id.is_empty() {
        let detail = project_service::detail(&state.pool, &state.cfg, user, &project_id).await?;
        return Ok((detail.project.id, detail.project.version, detail.graph));
    }

    // 按名称复用已有项目，避免 Agent 反复调用时创建一堆同名项目
    let visible = project_service::list(&state.pool, user, &project_name).await?;
    if let Some(found) = visible.iter().find(|p| p.name == project_name && p.permission != "viewer") {
        let detail = project_service::detail(&state.pool, &state.cfg, user, &found.id).await?;
        return Ok((detail.project.id, detail.project.version, detail.graph));
    }

    let name = if project_name.is_empty() {
        "AI 生成的项目".to_string()
    } else {
        project_name
    };
    let created = project_service::create_with_graph(
        &state.pool,
        &state.cfg,
        user,
        &name,
        "由 AI Agent 通过 MCP 自动生成",
        Graph::default(),
    )
    .await?;
    Ok((created.id, created.version, Graph::default()))
}

/// ingest_attack_path：批量写入节点与链路
async fn ingest(state: &AppState, user: &AuthUser, args: &Value) -> Result<Value, AppError> {
    let nodes: Vec<IngestNode> =
        serde_json::from_value(args.get("nodes").cloned().unwrap_or_else(|| json!([])))
            .map_err(|e| AppError::BadRequest(format!("nodes 格式错误：{e}")))?;
    let edges: Vec<IngestEdge> =
        serde_json::from_value(args.get("edges").cloned().unwrap_or_else(|| json!([])))
            .map_err(|e| AppError::BadRequest(format!("edges 格式错误：{e}")))?;

    if nodes.is_empty() && edges.is_empty() {
        return Err(AppError::BadRequest("nodes 与 edges 不能同时为空".to_string()));
    }
    if nodes.len() > 500 || edges.len() > 1000 {
        return Err(AppError::BadRequest("单次导入规模过大".to_string()));
    }

    let target = json!({
        "project_id": arg_str(args, "project_id"),
        "project_name": arg_str(args, "project_name"),
    });
    let (project_id, version, graph) = locate(state, user, &target).await?;
    write_graph(state, user, &project_id, version, graph, nodes, edges).await
}

/// add_nodes / add_edges 的入口
async fn append(
    state: &AppState,
    user: &AuthUser,
    project_id: &str,
    nodes: Vec<IngestNode>,
    edges: Vec<IngestEdge>,
) -> Result<Value, AppError> {
    if project_id.is_empty() {
        return Err(AppError::BadRequest("project_id 不能为空".to_string()));
    }
    let target = json!({ "project_id": project_id });
    let (id, version, graph) = locate(state, user, &target).await?;
    write_graph(state, user, &id, version, graph, nodes, edges).await
}

/// 更新单个节点
async fn update_node(
    state: &AppState,
    user: &AuthUser,
    project_id: &str,
    node_id: &str,
    patch: &Value,
) -> Result<Value, AppError> {
    if project_id.is_empty() || node_id.is_empty() {
        return Err(AppError::BadRequest("project_id 与 node_id 不能为空".to_string()));
    }
    let detail = project_service::detail(&state.pool, &state.cfg, user, project_id).await?;
    let mut graph = detail.graph;
    let node = graph
        .nodes
        .iter_mut()
        .find(|n| n.id == node_id || n.data.label == node_id)
        .ok_or_else(|| AppError::NotFound("节点不存在".to_string()))?;

    let text = |key: &str| patch.get(key).and_then(|v| v.as_str()).map(|v| v.to_string());
    if let Some(v) = text("label") {
        node.data.label = v;
    }
    if let Some(v) = text("kind") {
        node.data.kind = v;
    }
    if let Some(v) = text("ip") {
        node.data.ip = v;
    }
    if let Some(v) = text("port") {
        node.data.port = v;
    }
    if let Some(v) = text("os") {
        node.data.os = v;
    }
    if let Some(v) = text("status") {
        node.data.status = v;
    }
    if let Some(v) = text("notes") {
        node.data.notes = v;
    }

    let result = project_service::save(
        &state.pool,
        &state.cfg,
        user,
        project_id,
        SaveProjectRequest {
            data: graph,
            base_version: Some(detail.project.version),
        },
    )
    .await?;

    Ok(json!({ "version": result.version, "actions": result.changes }))
}

/// 写入节点漏洞详情（复用保存流程，因此同样产生字段级审计）
async fn set_vulnerability(
    state: &AppState,
    user: &AuthUser,
    project_id: &str,
    node_id: &str,
    payload: &Value,
) -> Result<Value, AppError> {
    if project_id.is_empty() || node_id.is_empty() {
        return Err(AppError::BadRequest("project_id 与 node_id 不能为空".to_string()));
    }
    let detail = project_service::detail(&state.pool, &state.cfg, user, project_id).await?;
    let mut graph = detail.graph;
    let node = graph
        .nodes
        .iter_mut()
        .find(|n| n.id == node_id || n.data.label == node_id)
        .ok_or_else(|| AppError::NotFound("节点不存在".to_string()))?;

    let text = |key: &str| {
        payload
            .get(key)
            .and_then(|v| v.as_str())
            .map(|v| v.to_string())
    };

    // 没给 index 就追加一条新漏洞；给了就覆盖对应条目（保留原 id）
    let index = payload.get("index").and_then(|value| value.as_i64());
    let position = match index {
        Some(value) if value >= 0 && (value as usize) < node.data.vulns.len() => value as usize,
        _ => {
            node.data.vulns.push(crate::model::Vulnerability {
                id: crate::db::new_id("v_"),
                ..Default::default()
            });
            node.data.vulns.len().saturating_sub(1)
        }
    };

    let (risk, vuln_id, filled) = {
        let vuln = &mut node.data.vulns[position];
        if let Some(value) = text("name") {
            vuln.name = value;
        }
        if let Some(value) = text("description") {
            vuln.description = value;
        }
        if let Some(value) = text("risk") {
            vuln.risk = value.to_lowercase();
        }
        if let Some(value) = text("detail") {
            vuln.detail = value;
        }
        if let Some(value) = text("remediation") {
            vuln.remediation = value;
        }
        vuln.updated_at = Some(crate::service::now_ms());
        vuln.updated_by = user.display_name.clone();
        (vuln.risk.clone(), vuln.id.clone(), !vuln.is_blank())
    };
    let total = node.data.vulns.len();

    let result = project_service::save(
        &state.pool,
        &state.cfg,
        user,
        project_id,
        SaveProjectRequest {
            data: graph,
            base_version: Some(detail.project.version),
        },
    )
    .await?;

    Ok(json!({
        "version": result.version,
        "actions": result.changes,
        "node_id": node_id,
        "index": position,
        "vuln_id": vuln_id,
        "risk": risk,
        "filled": filled,
        "total": total,
        "editor_path": format!("/#/p/{project_id}")
    }))
}

/// 删除节点的一条漏洞记录
async fn remove_vulnerability(
    state: &AppState,
    user: &AuthUser,
    project_id: &str,
    node_id: &str,
    payload: &Value,
) -> Result<Value, AppError> {
    if project_id.is_empty() || node_id.is_empty() {
        return Err(AppError::BadRequest("project_id 与 node_id 不能为空".to_string()));
    }
    let index = payload
        .get("index")
        .and_then(|value| value.as_i64())
        .ok_or_else(|| AppError::BadRequest("index 不能为空".to_string()))?;

    let detail = project_service::detail(&state.pool, &state.cfg, user, project_id).await?;
    let mut graph = detail.graph;
    let node = graph
        .nodes
        .iter_mut()
        .find(|n| n.id == node_id || n.data.label == node_id)
        .ok_or_else(|| AppError::NotFound("节点不存在".to_string()))?;

    if index < 0 || index as usize >= node.data.vulns.len() {
        return Err(AppError::NotFound("漏洞记录不存在".to_string()));
    }
    let removed = node.data.vulns.remove(index as usize);
    let total = node.data.vulns.len();

    let result = project_service::save(
        &state.pool,
        &state.cfg,
        user,
        project_id,
        SaveProjectRequest {
            data: graph,
            base_version: Some(detail.project.version),
        },
    )
    .await?;

    Ok(json!({
        "version": result.version,
        "actions": result.changes,
        "removed": removed.name,
        "total": total,
        "editor_path": format!("/#/p/{project_id}")
    }))
}

/// 把新节点与链路合并进现有图，自动布局后保存
async fn write_graph(
    state: &AppState,
    user: &AuthUser,
    project_id: &str,
    version: i64,
    mut graph: Graph,
    nodes: Vec<IngestNode>,
    edges: Vec<IngestEdge>,
) -> Result<Value, AppError> {
    // 名称 → 既有节点 id，便于 Agent 用 label 引用
    let mut by_label: HashMap<String, String> = graph
        .nodes
        .iter()
        .map(|n| (n.data.label.clone(), n.id.clone()))
        .collect();
    let mut external: HashMap<String, String> = HashMap::new();
    let mut added_nodes = 0usize;

    // 记录已有节点的最大 x，新增节点从右侧排布
    let base_x = graph
        .nodes
        .iter()
        .map(|n| n.position.x)
        .fold(0.0_f64, f64::max)
        + 320.0;

    let mut pending: Vec<GraphNode> = Vec::new();
    for item in nodes {
        let label = if item.label.trim().is_empty() {
            "未命名资产".to_string()
        } else {
            item.label.trim().to_string()
        };

        if let Some(existing_id) = by_label.get(&label) {
            if !item.id.is_empty() {
                external.insert(item.id.clone(), existing_id.clone());
            }
            continue;
        }

        let id = new_id("n_");
        if !item.id.is_empty() {
            external.insert(item.id.clone(), id.clone());
        }
        by_label.insert(label.clone(), id.clone());

        pending.push(GraphNode {
            id,
            position: Position { x: 0.0, y: 0.0 },
            data: NodeData {
                label,
                kind: if item.kind.is_empty() {
                    "host".to_string()
                } else {
                    item.kind
                },
                ip: item.ip,
                port: item.port,
                os: item.os,
                status: if item.status.is_empty() {
                    "recon".to_string()
                } else {
                    item.status
                },
                domain: String::new(),
                notes: item.notes,
                fields: Vec::new(),
                vulns: Vec::new(),
            },
            node_type: "asset".to_string(),
        });
        added_nodes += 1;
    }

    // 分层布局：按入边数量决定列，同列按行排开
    let mut extra_edges: Vec<(String, String)> = Vec::new();
    for edge in edges.iter() {
        let from = external
            .get(&edge.from)
            .or_else(|| by_label.get(&edge.from))
            .cloned();
        let to = external
            .get(&edge.to)
            .or_else(|| by_label.get(&edge.to))
            .cloned();
        if let (Some(f), Some(t)) = (from, to) {
            extra_edges.push((f, t));
        }
    }

    let layers = assign_layers(
        &pending.iter().map(|n| n.id.clone()).collect::<Vec<_>>(),
        &extra_edges,
    );
    let mut row_of_layer: HashMap<usize, usize> = HashMap::new();
    for mut node in pending {
        let layer = *layers.get(&node.id).unwrap_or(&0);
        let row = row_of_layer.entry(layer).or_insert(0);
        node.position = Position {
            x: base_x + (layer as f64) * 300.0,
            y: (*row as f64) * 140.0,
        };
        *row += 1;
        graph.nodes.push(node);
    }

    let mut added_edges = 0usize;
    let mut seen_pairs: HashSet<(String, String)> = graph
        .edges
        .iter()
        .map(|e| (e.source.clone(), e.target.clone()))
        .collect();

    for edge in edges.into_iter() {
        let from = external
            .get(&edge.from)
            .or_else(|| by_label.get(&edge.from))
            .cloned();
        let to = external
            .get(&edge.to)
            .or_else(|| by_label.get(&edge.to))
            .cloned();
        let (Some(source), Some(target)) = (from, to) else {
            continue;
        };
        if source == target || !seen_pairs.insert((source.clone(), target.clone())) {
            continue;
        }

        graph.edges.push(GraphEdge {
            id: new_id("e_"),
            source,
            target,
            label: edge.label,
            data: EdgeData {
                status: if edge.status.is_empty() {
                    "planned".to_string()
                } else {
                    edge.status
                },
                notes: String::new(),
                fields: Vec::new(),
                animated: false,
            },
            marker_end: "arrowclosed".to_string(),
        });
        added_edges += 1;
    }

    // 保护：这次调用什么都没加进来时不要落盘。
    // 空写入会把整图覆盖成空（历史拓扑就这样丢过一次），宁可报错也不写。
    if added_nodes == 0 && added_edges == 0 {
        return Err(AppError::BadRequest(
            "没有可写入的节点或链路（检查 label / from / to 是否有效）".to_string(),
        ));
    }

    let result = project_service::save(
        &state.pool,
        &state.cfg,
        user,
        project_id,
        SaveProjectRequest {
            data: graph,
            base_version: Some(version),
        },
    )
    .await?;

    Ok(json!({
        "project_id": project_id,
        "version": result.version,
        "added_nodes": added_nodes,
        "added_edges": added_edges,
        "node_total": result.node_count,
        "edge_total": result.edge_count,
        "id_map": external,
        "editor_path": format!("/#/p/{project_id}"),
    }))
}

/// 简化分层：按入边做一遍松弛，最多迭代 6 轮
fn assign_layers(ids: &[String], edges: &[(String, String)]) -> HashMap<String, usize> {
    let mut layers: HashMap<String, usize> = ids.iter().map(|id| (id.clone(), 0)).collect();
    for _ in 0..6 {
        let mut changed = false;
        for (from, to) in edges {
            let next = layers.get(from).copied().unwrap_or(0) + 1;
            let current = layers.get(to).copied().unwrap_or(0);
            if next > current && next < 6 {
                layers.insert(to.clone(), next);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    layers
}