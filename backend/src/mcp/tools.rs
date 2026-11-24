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
            "description": "更新单个节点的字段（名称、IP、域名、状态、备注、自定义属性）",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": { "type": "string" },
                    "node_id": { "type": "string" },
                    "patch": {
                        "type": "object",
                        "description": "只传要改的键。label/kind/ip/port/domain/os/status/notes 是字符串；fields 是自定义属性数组 [{key,value}]，整组替换，传 [] 清空"
                    }
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
                    "detail": { "type": "string", "description": "渗透测试详情。可直接写 Markdown：**粗体**、*斜体*、`代码`、- 无序列表、1. 有序列表、![说明](图片地址)，后端自动转成 HTML；写 HTML 会原样保留。插图先调 upload_image 拿 url" },
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
            "name": "upload_image",
            "description": "上传一张图片给渗透测试详情用。返回带签名的短地址，直接写进 detail 的 <img src>。支持 PNG / JPEG / WebP / GIF / SVG，单张不超过 2MB。",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "filename": { "type": "string", "description": "文件名，仅用于展示" },
                    "mime": { "type": "string", "description": "image/png、image/jpeg、image/webp、image/gif、image/svg+xml，默认 image/png" },
                    "data_base64": { "type": "string", "description": "图片的 base64，不带 data: 前缀" }
                },
                "required": ["data_base64"]
            }
        }),        json!({
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
            "name": "export_report",
            "description": "导出渗透测试报告。format=word 生成 .docx，pdf 生成 PDF，png 生成一张长图预览。不传 node_id 时导出整个项目所有节点的漏洞，按节点分节。返回可下载地址。",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": { "type": "string" },
                    "node_id": { "type": "string", "description": "只导出这一个节点；省略则导出整个项目" },
                    "format": { "type": "string", "enum": ["word", "pdf", "png"] }
                },
                "required": ["project_id", "format"]
            }
        }),
        json!({
            "name": "export_topology_png",
            "description": "把画布拓扑导出成 PNG：节点卡片、链路箭头、资产类型配色，2 倍图。返回可下载地址。",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": { "type": "string" },
                    "title": { "type": "string", "description": "图上的标题，默认用项目名" }
                },
                "required": ["project_id"]
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

fn arg_i64(args: &Value, key: &str) -> Option<i64> {
    args.get(key).and_then(|v| v.as_i64())
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
        "upload_image" => upload_image(state, user, args).await,
        "remove_vulnerability" => {
            let project_id = arg_str(args, "project_id");
            let node_id = arg_str(args, "node_id");
            remove_vulnerability(state, user, &project_id, &node_id, args).await
        }
        "search_graph" => {
            let project_id = arg_str(args, "project_id");
            let keyword = arg_str(args, "keyword").to_lowercase();
            let detail = project_service::detail(&state.pool, &state.cfg, user, &project_id).await?;
            let limit = arg_i64(args, "limit").unwrap_or(20).clamp(1, 200) as usize;

            let mut hits: Vec<Value> = Vec::new();
            for node in &detail.graph.nodes {
                // 命中字段逐个记下来，调用方一眼看出为什么匹配
                let mut matched: Vec<&str> = Vec::new();
                if keyword.is_empty() {
                    matched.push("all");
                } else {
                    if node.data.label.to_lowercase().contains(&keyword) {
                        matched.push("label");
                    }
                    if node.data.ip.to_lowercase().contains(&keyword) {
                        matched.push("ip");
                    }
                    if node.data.domain.to_lowercase().contains(&keyword) {
                        matched.push("domain");
                    }
                    if node.data.os.to_lowercase().contains(&keyword) {
                        matched.push("os");
                    }
                    if node.data.notes.to_lowercase().contains(&keyword) {
                        matched.push("notes");
                    }
                    if node.data.kind.to_lowercase().contains(&keyword) {
                        matched.push("kind");
                    }
                    for field in &node.data.fields {
                        if field.key.to_lowercase().contains(&keyword)
                            || field.value.to_lowercase().contains(&keyword)
                        {
                            matched.push("fields");
                            break;
                        }
                    }
                    // 漏洞名与说明也算命中，渗透场景里常按漏洞反查资产
                    for vuln in &node.data.vulns {
                        if vuln.name.to_lowercase().contains(&keyword)
                            || vuln.description.to_lowercase().contains(&keyword)
                        {
                            matched.push("vulns");
                            break;
                        }
                    }
                }
                if matched.is_empty() {
                    continue;
                }

                // 相关链路也带上，省一次 get_project
                let links: Vec<Value> = detail
                    .graph
                    .edges
                    .iter()
                    .filter(|e| e.source == node.id || e.target == node.id)
                    .map(|e| {
                        json!({
                            "id": e.id,
                            "from": e.source,
                            "to": e.target,
                            "label": e.label,
                        })
                    })
                    .collect();

                hits.push(json!({
                    "id": node.id,
                    "label": node.data.label,
                    "kind": node.data.kind,
                    "ip": node.data.ip,
                    "port": node.data.port,
                    "domain": node.data.domain,
                    "os": node.data.os,
                    "status": node.data.status,
                    "notes": node.data.notes,
                    "position": { "x": node.position.x, "y": node.position.y },
                    "fields": node.data.fields.iter().map(|f| json!({"key": f.key, "value": f.value})).collect::<Vec<_>>(),
                    "vulnCount": node.data.vulns.len(),
                    "vulns": node.data.vulns.iter().map(|v| json!({
                        "name": v.name,
                        "risk": v.risk,
                        "description": v.description,
                    })).collect::<Vec<_>>(),
                    "edges": links,
                    "matched": matched,
                }));
            }

            // 名称精确命中的排前面，其次按名称长度（越短越相关）
            hits.sort_by(|a, b| {
                let score = |v: &Value| -> (u8, usize) {
                    let name = v["label"].as_str().unwrap_or("").to_lowercase();
                    let exact = if !keyword.is_empty() && name == keyword { 0 } else if name.contains(&keyword) { 1 } else { 2 };
                    (exact, name.chars().count())
                };
                score(a).cmp(&score(b))
            });

            let total = hits.len();
            hits.truncate(limit);
            Ok(json!({ "count": hits.len(), "total": total, "nodes": hits }))
        }
        "export_topology_png" => {
            let project_id = arg_str(args, "project_id");
            let detail = project_service::detail(&state.pool, &state.cfg, user, &project_id).await?;
            let mut title = arg_str(args, "title");
            if title.is_empty() {
                title = detail.project.name.clone();
            }
            let bytes = crate::mcp::export::canvas_png(&detail.graph, &title)?;
            let url = crate::mcp::export::publish(
                &state.cfg,
                &bytes,
                "png",
                &format!("{}-拓扑", detail.project.name),
            )
            .await?;
            Ok(json!({
                "format": "png",
                "bytes": bytes.len(),
                "nodes": detail.graph.nodes.len(),
                "edges": detail.graph.edges.len(),
                "url": url,
                "download": format!("{url}"),
            }))
        }
        "export_report" => {
            let project_id = arg_str(args, "project_id");
            let node_id = arg_str(args, "node_id");
            let format = {
                let raw = arg_str(args, "format").to_lowercase();
                if raw.is_empty() { "word".to_string() } else { raw }
            };
            if !matches!(format.as_str(), "word" | "pdf" | "png") {
                return Err(AppError::BadRequest("format 只支持 word / pdf / png".into()));
            }
            let detail = project_service::detail(&state.pool, &state.cfg, user, &project_id).await?;
            let images = crate::mcp::export::ImageStore::new(detail_upload_root(&state.cfg));

            // 选定要导出的节点：指定了就只导它，否则导全部有漏洞的节点
            let targets: Vec<&crate::model::GraphNode> = if node_id.is_empty() {
                detail
                    .graph
                    .nodes
                    .iter()
                    .filter(|n| !n.data.vulns.is_empty())
                    .collect()
            } else {
                let found = detail
                    .graph
                    .nodes
                    .iter()
                    .filter(|n| n.id == node_id || n.data.label == node_id)
                    .collect::<Vec<_>>();
                if found.is_empty() {
                    return Err(AppError::NotFound(format!("节点 {node_id} 不存在")));
                }
                found
            };

            let generated_at = crate::service::now_ms();
            let stamp = format_time(generated_at);
            let author = user.display_name.clone();

            let mut sections: Vec<(crate::mcp::render::ReportMeta, Vec<crate::model::Vulnerability>)> = Vec::new();
            for node in &targets {
                sections.push((
                    crate::mcp::render::ReportMeta {
                        project: detail.project.name.clone(),
                        node: if node.data.label.is_empty() { "未命名资产".into() } else { node.data.label.clone() },
                        kind: node.data.kind.clone(),
                        ip: node.data.ip.clone(),
                        port: node.data.port.clone(),
                        domain: node.data.domain.clone(),
                        status: node.data.status.clone(),
                        notes: node.data.notes.clone(),
                        generated_at: stamp.clone(),
                        author: author.clone(),
                    },
                    node.data.vulns.clone(),
                ));
            }
            if sections.is_empty() {
                return Err(AppError::BadRequest("该项目还没有任何漏洞记录，无需导出".into()));
            }

            // 多节点时把内容拼成一份：元信息用项目级，漏洞按节点累加
            let meta = sections[0].0.clone();
            let mut all_vulns: Vec<crate::model::Vulnerability> = Vec::new();
            if sections.len() > 1 {
                for (sec_meta, vulns) in &sections {
                    let mut head = crate::model::Vulnerability {
                        name: format!("【{}】", sec_meta.node),
                        ..Default::default()
                    };
                    head.risk = String::new();
                    all_vulns.push(head);
                    all_vulns.extend(vulns.iter().cloned());
                }
            } else {
                all_vulns = sections[0].1.clone();
            }

            let stem = if node_id.is_empty() {
                format!("{}-渗透测试报告", detail.project.name)
            } else {
                format!("{}-{}-漏洞报告", detail.project.name, sections[0].0.node)
            };

            let (bytes, ext, mime) = match format.as_str() {
                "word" => (
                    crate::mcp::export::report_word(&meta, &all_vulns, &images)?,
                    "docx",
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                ),
                "pdf" => (
                    crate::mcp::export::report_pdf(&meta, &all_vulns, &images)?,
                    "pdf",
                    "application/pdf",
                ),
                _ => (
                    crate::mcp::export::report_png(&meta, &all_vulns, &images)?,
                    "png",
                    "image/png",
                ),
            };

            let url = crate::mcp::export::publish(&state.cfg, &bytes, ext, &stem).await?;
            Ok(json!({
                "format": format,
                "mime": mime,
                "bytes": bytes.len(),
                "nodes": sections.len(),
                "vulns": all_vulns.len(),
                "url": url,
            }))
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
    if let Some(v) = text("domain") {
        node.data.domain = v;
    }

    // 自定义属性：整组替换。传 [{key, value}]，传空数组表示清空，不传则保持原样。
    if let Some(list) = patch.get("fields").and_then(|v| v.as_array()) {
        node.data.fields = list
            .iter()
            .filter_map(|item| {
                let key = item.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let value = item.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string();
                (!key.is_empty() || !value.is_empty()).then(|| crate::model::graph::Field { key, value })
            })
            .collect();
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
            // Agent 通常写 Markdown；已经是 HTML 的原样保留
            vuln.detail = super::richtext::markdown_to_html(&value);
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
/// 上传一张图片，返回可直接写进 detail 的签名地址
async fn upload_image(state: &AppState, user: &AuthUser, payload: &Value) -> Result<Value, AppError> {
    let data = arg_str(payload, "data_base64");
    if data.trim().is_empty() {
        return Err(AppError::BadRequest("data_base64 不能为空".to_string()));
    }
    let mime = {
        let raw = arg_str(payload, "mime");
        let raw = raw.trim();
        if raw.is_empty() {
            "image/png".to_string()
        } else {
            raw.to_string()
        }
    };
    let filename = {
        let raw = arg_str(payload, "filename");
        let raw = raw.trim();
        if raw.is_empty() {
            "mcp-upload".to_string()
        } else {
            raw.to_string()
        }
    };

    let result = crate::service::asset_service::save_upload(
        &state.pool,
        &state.cfg,
        user,
        crate::model::misc::UploadRequest {
            filename,
            mime,
            data_base64: data,
        },
    )
    .await?;

    Ok(json!({
        "id": result.id,
        "url": result.url,
        "bytes": result.bytes,
        "mime": result.mime,
        "usage": format!("<img src=\"{}\">", result.url),
    }))
}
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
/// 上传目录：导出时的图片来源
fn detail_upload_root(cfg: &crate::config::Config) -> std::path::PathBuf {
    cfg.upload_dir.clone()
}

/// 毫秒时间戳转成「2026-11-24 11:42」这种给人看的写法
fn format_time(ms: i64) -> String {
    // 时间戳是 UTC，展示时换算到东八区
    let secs = ms.div_euclid(1000) + 8 * 3600;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, m) = (rem / 3600, (rem % 3600) / 60);

    // 从 1970-01-01 起算的民用历，够用且不引 chrono
    let mut year = 1970i64;
    let mut left = days;
    loop {
        let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
        let len = if leap { 366 } else { 365 };
        if left < len {
            break;
        }
        left -= len;
        year += 1;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let months = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 0usize;
    while month < 12 && left >= months[month] as i64 {
        left -= months[month] as i64;
        month += 1;
    }
    format!("{year:04}-{:02}-{:02} {h:02}:{m:02}", month + 1, left + 1)
}