//! 拓扑规范化与差异计算（纯函数，无 IO）。

use std::collections::HashMap;

use crate::model::{Change, EdgeData, FieldChange, Graph, GraphEdge, GraphNode, NodeData, Vulnerability};

const MAX_NODES: usize = 20_000;
const MAX_EDGES: usize = 40_000;
const MAX_FIELDS: usize = 40;

const NODE_STATUS: [&str; 4] = ["recon", "attack", "control", "blocked"];
const EDGE_STATUS: [&str; 4] = ["planned", "active", "success", "failed"];

fn node_status_label(value: &str) -> &'static str {
    match value {
        "attack" => "攻击中",
        "control" => "已控制",
        "blocked" => "受阻中",
        _ => "侦察中",
    }
}

/// 旧版本的状态值映射到四档，避免历史拓扑打开后变成「未知」
fn migrate_status(value: &str) -> &'static str {
    match value {
        "vuln" | "attacking" | "attack" => "attack",
        "owned" | "control" => "control",
        "blocked" => "blocked",
        _ => "recon",
    }
}

fn edge_status_label(value: &str) -> &'static str {
    match value {
        "active" => "进行中",
        "success" => "已成功",
        "failed" => "失败",
        _ => "计划中",
    }
}

/// 规范化字段列表：去空、截断、限制条数
/// 风险等级取值（与前端、报告一致）
const RISK_LEVELS: [&str; 5] = ["critical", "high", "medium", "low", "info"];

/// 风险等级中文名，用于审计与报告
pub fn risk_label(value: &str) -> &'static str {
    match value {
        "critical" => "严重",
        "high" => "高危",
        "medium" => "中危",
        "low" => "低危",
        "info" => "信息",
        _ => "未定级",
    }
}

/// 单个资产最多挂多少条漏洞
const MAX_VULNS: usize = 50;

/// 规整一条漏洞：裁剪长度、校验等级、补 id
fn normalize_vuln(input: &crate::model::Vulnerability) -> crate::model::Vulnerability {
    let risk = if RISK_LEVELS.contains(&input.risk.as_str()) {
        input.risk.clone()
    } else {
        String::new()
    };
    let id = if super::valid_vuln_id(&input.id) {
        input.id.clone()
    } else {
        crate::db::new_id("v_")
    };
    crate::model::Vulnerability {
        id,
        name: super::tidy(&input.name, 120),
        description: super::clamp(&input.description, 8000),
        risk,
        // 富文本详情内联图片，一张压缩图约占 30 万字符；上限必须留足，否则图片会被切掉
        detail: super::clamp(&input.detail, 4_000_000),
        remediation: super::clamp(&input.remediation, 8000),
        images: input
            .images
            .iter()
            .filter(|id| super::asset_service::valid_upload_id(id))
            .take(8)
            .cloned()
            .collect(),
        updated_at: input.updated_at,
        updated_by: super::tidy(&input.updated_by, 40),
    }
}

/// 规整漏洞列表：丢弃全空条目，最多保留 50 条
fn normalize_vulns(input: &[crate::model::Vulnerability]) -> Vec<crate::model::Vulnerability> {
    input
        .iter()
        .map(normalize_vuln)
        .filter(|item| !item.is_blank())
        .take(MAX_VULNS)
        .collect()
}

/// 长文本在审计里只保留一行摘要
fn brief(text: &str) -> String {
    let one_line = text.replace(['\r', '\n'], " ");
    if one_line.chars().count() > 60 {
        let head: String = one_line.chars().take(60).collect();
        format!("{head}…")
    } else {
        one_line
    }
}

fn normalize_fields(fields: &[crate::model::Field]) -> Vec<crate::model::Field> {
    fields
        .iter()
        .filter(|f| !f.key.trim().is_empty() || !f.value.trim().is_empty())
        .take(MAX_FIELDS)
        .map(|f| crate::model::Field {
            key: super::tidy(&f.key, 40),
            value: super::clamp(&f.value, 400),
        })
        .collect()
}

/// 规范化整张图：限制规模、统一状态枚举、剔除悬空连线
pub fn normalize(input: Graph) -> Graph {
    let mut seen_nodes: HashMap<String, ()> = HashMap::new();
    let mut nodes: Vec<GraphNode> = Vec::new();

    for node in input.nodes.into_iter().take(MAX_NODES) {
        let id = node.id.trim().to_string();
        if id.is_empty() || id.len() > 64 || seen_nodes.contains_key(&id) {
            continue;
        }
        seen_nodes.insert(id.clone(), ());

        // 旧状态（unknown / vuln / attacking / owned）在这里折叠到四档
        let status = if NODE_STATUS.contains(&node.data.status.as_str()) {
            node.data.status.clone()
        } else {
            migrate_status(&node.data.status).to_string()
        };

        nodes.push(GraphNode {
            id,
            position: crate::model::Position {
                x: round2(node.position.x),
                y: round2(node.position.y),
            },
            data: NodeData {
                label: super::tidy(&node.data.label, 80),
                kind: super::tidy(&node.data.kind, 40),
                ip: super::tidy(&node.data.ip, 64),
                port: super::tidy(&node.data.port, 12),
                domain: super::tidy(&node.data.domain, 120),
                os: super::tidy(&node.data.os, 80),
                status,
                notes: super::clamp(&node.data.notes, 4000),
                fields: normalize_fields(&node.data.fields),
                vulns: normalize_vulns(&node.data.vulns),
            },
            node_type: "asset".to_string(),
        });
    }

    let mut seen_edges: HashMap<String, ()> = HashMap::new();
    let mut edges: Vec<GraphEdge> = Vec::new();
    for edge in input.edges.into_iter().take(MAX_EDGES) {
        let id = edge.id.trim().to_string();
        if id.is_empty() || id.len() > 64 || seen_edges.contains_key(&id) {
            continue;
        }
        // 悬空连线直接丢弃，避免前端渲染时崩溃
        if !seen_nodes.contains_key(edge.source.trim()) || !seen_nodes.contains_key(edge.target.trim()) {
            continue;
        }
        if edge.source == edge.target {
            continue;
        }
        seen_edges.insert(id.clone(), ());

        let status = if EDGE_STATUS.contains(&edge.data.status.as_str()) {
            edge.data.status.clone()
        } else {
            "planned".to_string()
        };

        edges.push(GraphEdge {
            id,
            source: edge.source.trim().to_string(),
            target: edge.target.trim().to_string(),
            label: String::new(),
            data: EdgeData {
                status,
                notes: super::clamp(&edge.data.notes, 2000),
                fields: normalize_fields(&edge.data.fields),
                animated: edge.data.animated,
            },
            marker_end: if edge.marker_end.is_empty() {
                "arrowclosed".to_string()
            } else {
                super::tidy(&edge.marker_end, 32)
            },
        });
    }

    let zoom = if input.viewport.zoom.is_finite() && input.viewport.zoom > 0.0 {
        input.viewport.zoom.clamp(0.1, 3.0)
    } else {
        1.0
    };

    Graph {
        nodes,
        edges,
        viewport: crate::model::Viewport {
            x: round2(input.viewport.x),
            y: round2(input.viewport.y),
            zoom,
        },
    }
}

fn round2(v: f64) -> f64 {
    if v.is_finite() {
        (v * 100.0).round() / 100.0
    } else {
        0.0
    }
}

/// 比较两个字段列表，产出「字段名 旧值→新值」
fn diff_fields(before: &[crate::model::Field], after: &[crate::model::Field]) -> Vec<FieldChange> {
    let mut out = Vec::new();
    let to_map = |list: &[crate::model::Field]| {
        list.iter()
            .map(|f| (f.key.clone(), f.value.clone()))
            .collect::<HashMap<_, _>>()
    };
    let a = to_map(before);
    let b = to_map(after);
    for (key, value) in b.iter() {
        match a.get(key) {
            Some(old) if old == value => {}
            Some(old) => out.push(FieldChange {
                field: if key.is_empty() { "字段".to_string() } else { key.clone() },
                from: old.clone(),
                to: value.clone(),
            }),
            None => out.push(FieldChange {
                field: if key.is_empty() { "字段".to_string() } else { key.clone() },
                from: String::new(),
                to: value.clone(),
            }),
        }
    }
    for (key, value) in a.iter() {
        if !b.contains_key(key) {
            out.push(FieldChange {
                field: if key.is_empty() { "字段".to_string() } else { key.clone() },
                from: value.clone(),
                to: String::new(),
            });
        }
    }
    out
}

/// 比较节点业务字段
fn diff_node(before: &NodeData, after: &NodeData) -> Vec<FieldChange> {
    let mut out = Vec::new();
    let pairs = [
        ("名称", before.label.clone(), after.label.clone()),
        ("类型", before.kind.clone(), after.kind.clone()),
        ("IP", before.ip.clone(), after.ip.clone()),
        ("端口", before.port.clone(), after.port.clone()),
        ("系统", before.os.clone(), after.os.clone()),
        (
            "状态",
            node_status_label(&before.status).to_string(),
            node_status_label(&after.status).to_string(),
        ),
        ("备注", before.notes.clone(), after.notes.clone()),
        ("域名", before.domain.clone(), after.domain.clone()),
    ];
    for (field, from, to) in pairs {
        if from != to {
            out.push(FieldChange {
                field: field.to_string(),
                from,
                to,
            });
        }
    }
    out.extend(diff_fields(&before.fields, &after.fields));

    out.extend(diff_vulns(&before.vulns, &after.vulns));
    out
}

/// 漏洞列表的差异：按 id 配对，新增/删除整条记录，修改按字段出摘要
fn diff_vulns(before: &[Vulnerability], after: &[Vulnerability]) -> Vec<FieldChange> {
    let mut out = Vec::new();
    let title = |item: &Vulnerability| {
        if item.name.trim().is_empty() {
            "未命名漏洞".to_string()
        } else {
            brief(&item.name)
        }
    };

    for item in after {
        if !before.iter().any(|old| old.id == item.id) {
            out.push(FieldChange {
                field: "新增漏洞".to_string(),
                from: String::new(),
                to: title(item),
            });
        }
    }
    for item in before {
        if !after.iter().any(|new| new.id == item.id) {
            out.push(FieldChange {
                field: "删除漏洞".to_string(),
                from: title(item),
                to: String::new(),
            });
        }
    }
    for item in after {
        let Some(old) = before.iter().find(|old| old.id == item.id) else {
            continue;
        };
        let name = title(item);
        // 只有名称参与审计：说明、详情、建议是正文内容，逐字比对既没用又会把图片 base64 塞进记录
        let pairs = [("漏洞名称", old.name.clone(), item.name.clone())];
        for (field, from, to) in pairs {
            if from != to {
                out.push(FieldChange {
                    field: format!("{field}（{name}）"),
                    from: brief(from.as_str()),
                    to: brief(to.as_str()),
                });
            }
        }
        if old.detail != item.detail {
            out.push(FieldChange {
                field: format!("渗透详情（{name}）"),
                from: String::new(),
                to: "已更新".to_string(),
            });
        }
        if old.remediation != item.remediation {
            out.push(FieldChange {
                field: format!("处置建议（{name}）"),
                from: String::new(),
                to: "已更新".to_string(),
            });
        }
        if old.description != item.description {
            out.push(FieldChange {
                field: format!("漏洞说明（{name}）"),
                from: String::new(),
                to: "已更新".to_string(),
            });
        }
        if old.risk != item.risk {
            out.push(FieldChange {
                field: format!("风险等级（{name}）"),
                from: risk_label(&old.risk).to_string(),
                to: risk_label(&item.risk).to_string(),
            });
        }
    }
    out
}

/// 比较连线业务字段
fn diff_edge(before: &EdgeData, after: &EdgeData, before_label: &str, after_label: &str) -> Vec<FieldChange> {
    let mut out = Vec::new();
    if before_label != after_label {
        out.push(FieldChange {
            field: "标签".to_string(),
            from: before_label.to_string(),
            to: after_label.to_string(),
        });
    }
    let pairs = [
        ("备注", before.notes.clone(), after.notes.clone()),
        (
            "状态",
            edge_status_label(&before.status).to_string(),
            edge_status_label(&after.status).to_string(),
        ),
        ("备注", before.notes.clone(), after.notes.clone()),
    ];
    for (field, from, to) in pairs {
        if from != to {
            out.push(FieldChange {
                field: field.to_string(),
                from,
                to,
            });
        }
    }
    out.extend(diff_fields(&before.fields, &after.fields));
    out
}

/// 计算两次拓扑之间的变更集合，用于生成审计
pub fn diff(before: &Graph, after: &Graph) -> Vec<Change> {
    let mut changes: Vec<Change> = Vec::new();

    let before_nodes: HashMap<&str, &GraphNode> =
        before.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let after_nodes: HashMap<&str, &GraphNode> =
        after.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    for node in after.nodes.iter() {
        let label = if node.data.label.is_empty() {
            "未命名节点".to_string()
        } else {
            node.data.label.clone()
        };
        match before_nodes.get(node.id.as_str()) {
            None => changes.push(Change {
                action: "node.create".to_string(),
                target_type: "node".to_string(),
                target_id: node.id.clone(),
                target_label: label.clone(),
                summary: format!("新增 {label} 节点"),
                changes: Vec::new(),
            }),
            Some(old) => {
                let fields = diff_node(&old.data, &node.data);
                if !fields.is_empty() {
                    let brief = fields
                        .iter()
                        .take(3)
                        .map(|f| format!("{} {} 改为 {}", f.field, dash(&f.from), dash(&f.to)))
                        .collect::<Vec<_>>()
                        .join("；");
                    changes.push(Change {
                        action: "node.update".to_string(),
                        target_type: "node".to_string(),
                        target_id: node.id.clone(),
                        target_label: label.clone(),
                        summary: format!("修改 {label} 节点：{brief}"),
                        changes: fields,
                    });
                }
            }
        }
    }

    for node in before.nodes.iter() {
        if !after_nodes.contains_key(node.id.as_str()) {
            let label = if node.data.label.is_empty() {
                "未命名节点".to_string()
            } else {
                node.data.label.clone()
            };
            changes.push(Change {
                action: "node.delete".to_string(),
                target_type: "node".to_string(),
                target_id: node.id.clone(),
                target_label: label.clone(),
                summary: format!("删除 {label} 节点"),
                changes: Vec::new(),
            });
        }
    }

    let before_edges: HashMap<&str, &GraphEdge> =
        before.edges.iter().map(|e| (e.id.as_str(), e)).collect();
    let after_edges: HashMap<&str, &GraphEdge> =
        after.edges.iter().map(|e| (e.id.as_str(), e)).collect();

    for edge in after.edges.iter() {
        let label = edge_label(edge);
        match before_edges.get(edge.id.as_str()) {
            None => changes.push(Change {
                action: "edge.create".to_string(),
                target_type: "edge".to_string(),
                target_id: edge.id.clone(),
                target_label: label.clone(),
                summary: format!("新增 {label} 链路"),
                changes: Vec::new(),
            }),
            Some(old) => {
                let fields = diff_edge(&old.data, &edge.data, &old.label, &edge.label);
                if !fields.is_empty() {
                    let brief = fields
                        .iter()
                        .take(3)
                        .map(|f| format!("{} {} 改为 {}", f.field, dash(&f.from), dash(&f.to)))
                        .collect::<Vec<_>>()
                        .join("；");
                    changes.push(Change {
                        action: "edge.update".to_string(),
                        target_type: "edge".to_string(),
                        target_id: edge.id.clone(),
                        target_label: label.clone(),
                        summary: format!("修改 {label} 链路：{brief}"),
                        changes: fields,
                    });
                }
            }
        }
    }

    for edge in before.edges.iter() {
        if !after_edges.contains_key(edge.id.as_str()) {
            let label = edge_label(edge);
            changes.push(Change {
                action: "edge.delete".to_string(),
                target_type: "edge".to_string(),
                target_id: edge.id.clone(),
                target_label: label.clone(),
                summary: format!("删除 {label} 链路"),
                changes: Vec::new(),
            });
        }
    }

    changes
}

/// 连线展示名
fn edge_label(edge: &GraphEdge) -> String {
    if !edge.label.trim().is_empty() {
        return edge.label.clone();
    }
    "未命名链路".to_string()
}

/// 空值占位
fn dash(value: &str) -> String {
    if value.trim().is_empty() {
        "空".to_string()
    } else {
        super::clamp(value, 24)
    }
}