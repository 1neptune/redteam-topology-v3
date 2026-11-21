import type { FieldChange, Graph } from './graph'

/** 项目权限 */
export type Permission = 'owner' | 'editor' | 'viewer'

/** 列表项 */
/** 项目状态：进行中 / 已完成 */
export type ProjectStatus = 'active' | 'done'

export interface ProjectSummary {
  id: string
  name: string
  description: string
  ownerId: string
  version: number
  nodeCount: number
  edgeCount: number
  permission: Permission
  createdAt: number
  updatedAt: number
  updatedBy: string
  /** active（进行中）/ done（已完成） */
  status: ProjectStatus
}

/** 详情（含解密后的拓扑） */
export interface ProjectDetail {
  project: ProjectSummary
  permission: Permission
  graph: Graph
}

/** 保存结果 */
export interface SaveResult {
  version: number
  nodeCount: number
  edgeCount: number
  changes: number
  updatedAt: number
}

/** 成员条目 */
export interface MemberEntry {
  userId: string
  username: string
  displayName: string
  permission: Permission
  grantedAt: number
}

/** 审计记录 */
export interface AuditRecord {
  id: string
  projectId: string | null
  userName: string
  action: string
  targetType: string
  targetId: string
  targetLabel: string
  summary: string
  changes: FieldChange[]
  createdAt: number
}

/** 导出 bundle */
export interface ExportBundle {
  format: string
  schemaVersion: number
  exportedAt: number
  exportedBy: string
  project: {
    name: string
    description: string
    data: Graph
  }
}

/** 在线成员 */
export interface PresenceEntry {
  userId: string
  userName: string
  cursorX: number
  cursorY: number
  seenAt: number
}

export interface CreateProjectPayload {
  name: string
  description?: string
  data?: Graph
  status?: ProjectStatus
}

export interface SaveProjectPayload {
  data: Graph
  baseVersion?: number
}

export interface UpdateProjectPayload {
  name?: string
  description?: string
  status?: ProjectStatus
}

export interface GrantPayload {
  userId: string
  permission: Permission
}

export interface ImportPayload {
  project: {
    name: string
    description?: string
    data: Graph
  }
}