//! 拓扑数据结构与变更描述。

use serde::{Deserialize, Serialize};

/// 自定义字段（key/value）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub value: String,
}

/// 画布坐标
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Position {
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
}

/// 视口
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Viewport {
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default = "default_zoom")]
    pub zoom: f64,
}

fn default_zoom() -> f64 {
    1.0
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }
    }
}

/// 漏洞详情：双击节点打开的面板，也是导出 Word / PDF 报告的内容来源
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Vulnerability {
    /// 稳定标识，前端列表增删改与审计配对用
    #[serde(default)]
    pub id: String,
    /// 漏洞名称，如「任意文件上传漏洞」
    #[serde(default)]
    pub name: String,
    /// 漏洞说明
    #[serde(default)]
    pub description: String,
    /// 风险等级：critical / high / medium / low / info
    #[serde(default)]
    pub risk: String,
    /// 渗透测试详情（复现步骤、请求报文等）
    #[serde(default)]
    pub detail: String,
    /// 处置建议
    #[serde(default)]
    pub remediation: String,
    /// 渗透测试详情配图（upload id 列表）
    #[serde(default)]
    pub images: Vec<String>,
    /// 最后修改时间（毫秒）
    #[serde(default)]
    pub updated_at: Option<i64>,
    /// 最后修改人显示名
    #[serde(default)]
    pub updated_by: String,
}

impl Vulnerability {
    /// 五个业务字段是否全空（全空视为「没有漏洞详情」）
    pub fn is_blank(&self) -> bool {
        self.name.is_empty()
            && self.description.is_empty()
            && self.risk.is_empty()
            && self.detail.is_empty()
            && self.remediation.is_empty()
    }
}

/// 节点业务数据
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NodeData {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub ip: String,
    #[serde(default)]
    pub port: String,
    /// 域名或主机名（与 IP 并存，二者可只填一个）
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    pub os: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub fields: Vec<Field>,
    /// 漏洞列表（一个资产可挂多条）
    #[serde(default)]
    pub vulns: Vec<Vulnerability>,
}

/// 节点
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: String,
    #[serde(default)]
    pub position: Position,
    #[serde(default)]
    pub data: NodeData,
    #[serde(default, rename = "type")]
    pub node_type: String,
}

/// 连线业务数据
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EdgeData {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub fields: Vec<Field>,
    #[serde(default)]
    pub animated: bool,
}

/// 连线
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub data: EdgeData,
    #[serde(default)]
    pub marker_end: String,
}

/// 完整拓扑
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Graph {
    #[serde(default)]
    pub nodes: Vec<GraphNode>,
    #[serde(default)]
    pub edges: Vec<GraphEdge>,
    #[serde(default)]
    pub viewport: Viewport,
}

/// 单个字段的变更
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldChange {
    pub field: String,
    pub from: String,
    pub to: String,
}

/// 一次变更（对应一条审计记录）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub target_label: String,
    pub summary: String,
    pub changes: Vec<FieldChange>,
}