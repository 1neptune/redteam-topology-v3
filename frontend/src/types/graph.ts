/** 节点控制状态 */
export type NodeStatus = 'recon' | 'attack' | 'control' | 'blocked'

/** 链路状态 */
export type EdgeStatus = 'planned' | 'active' | 'success' | 'failed'

/** 自定义字段 */
export interface GraphField {
  key: string
  value: string
}

export interface GraphPosition {
  x: number
  y: number
}

export interface GraphViewport extends GraphPosition {
  zoom: number
}

/** 风险等级 */
export type RiskLevel = 'critical' | 'high' | 'medium' | 'low' | 'info'

/** 漏洞详情：双击节点打开的面板，也是导出 Word / PDF 报告的内容来源 */
export interface Vulnerability {
  /** 稳定标识，列表增删改与审计配对用 */
  id: string
  /** 漏洞名称，如「任意文件上传漏洞」 */
  name: string
  /** 漏洞说明 */
  description: string
  /** 风险等级，空串表示未定级 */
  risk: RiskLevel | ''
  /** 渗透测试详情 */
  detail: string
  /** 处置建议 */
  remediation: string
  /** 最后修改时间 */
  updatedAt?: number | null
  /** 最后修改人 */
  updatedBy?: string
}

/** 节点业务数据 */
export interface NodeData {
  label: string
  kind: string
  ip: string
  port: string
  /** 域名或主机名 */
  domain: string
  os: string
  status: NodeStatus
  notes: string
  fields: GraphField[]
  /** 漏洞列表（一个资产可挂多条） */
  vulns: Vulnerability[]
}

/** 画布节点 */
export interface GraphNode {
  id: string
  type: string
  position: GraphPosition
  data: NodeData
  selected?: boolean
  computedPosition?: GraphPosition
  dimensions?: { width: number; height: number }
}

export interface EdgeData {
  status: EdgeStatus
  notes: string
  fields: GraphField[]
  animated: boolean
}

/** 画布链路 */
export interface GraphEdge {
  id: string
  source: string
  target: string
  label: string
  type?: string
  markerEnd?: string
  data: EdgeData
  selected?: boolean
}

/** 完整拓扑 */
export interface Graph {
  nodes: GraphNode[]
  edges: GraphEdge[]
  viewport: GraphViewport
}

/** 字段级变更（审计展示） */
export interface FieldChange {
  field: string
  from: string
  to: string
}
