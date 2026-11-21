import { request } from './http'
import type {
  AuditRecord,
  CreateProjectPayload,
  ExportBundle,
  GrantPayload,
  ImportPayload,
  MemberEntry,
  PageQuery,
  PresenceEntry,
  ProjectDetail,
  ProjectSummary,
  SaveProjectPayload,
  SaveResult,
  UpdateProjectPayload,
} from '@/types'

/** 项目列表（服务端按权限过滤 + 关键字搜索） */
export function fetchProjects(keyword = ''): Promise<ProjectSummary[]> {
  return request<ProjectSummary[]>({
    url: '/projects',
    method: 'GET',
    params: keyword ? { q: keyword } : undefined,
  })
}

export function createProject(payload: CreateProjectPayload): Promise<ProjectSummary> {
  return request<ProjectSummary>({ url: '/projects', method: 'POST', data: payload })
}

export function fetchProject(id: string): Promise<ProjectDetail> {
  return request<ProjectDetail>({ url: `/projects/${encodeURIComponent(id)}`, method: 'GET' })
}

/** 全量保存拓扑（带版本号做乐观锁） */
export function saveProject(id: string, payload: SaveProjectPayload): Promise<SaveResult> {
  return request<SaveResult>({
    url: `/projects/${encodeURIComponent(id)}`,
    method: 'PUT',
    data: payload,
  })
}

/** 局部更新项目资料 */
export function patchProject(id: string, payload: UpdateProjectPayload): Promise<ProjectSummary> {
  return request<ProjectSummary>({
    url: `/projects/${encodeURIComponent(id)}`,
    method: 'PATCH',
    data: payload,
  })
}

export function deleteProject(id: string): Promise<null> {
  return request<null>({ url: `/projects/${encodeURIComponent(id)}`, method: 'DELETE' })
}

export function duplicateProject(id: string): Promise<ProjectSummary> {
  return request<ProjectSummary>({
    url: `/projects/${encodeURIComponent(id)}/duplicate`,
    method: 'POST',
  })
}

export function exportProject(id: string): Promise<ExportBundle> {
  return request<ExportBundle>({ url: `/projects/${encodeURIComponent(id)}/export`, method: 'GET' })
}

export function importProject(payload: ImportPayload): Promise<ProjectSummary> {
  return request<ProjectSummary>({ url: '/projects/import', method: 'POST', data: payload })
}

export function fetchMembers(id: string): Promise<MemberEntry[]> {
  return request<MemberEntry[]>({
    url: `/projects/${encodeURIComponent(id)}/members`,
    method: 'GET',
  })
}

export function grantMember(id: string, payload: GrantPayload): Promise<MemberEntry[]> {
  return request<MemberEntry[]>({
    url: `/projects/${encodeURIComponent(id)}/members`,
    method: 'POST',
    data: payload,
  })
}

export function revokeMember(id: string, userId: string): Promise<MemberEntry[]> {
  return request<MemberEntry[]>({
    url: `/projects/${encodeURIComponent(id)}/members/${encodeURIComponent(userId)}`,
    method: 'DELETE',
  })
}

export function fetchAudit(id: string, query: PageQuery = {}): Promise<AuditRecord[]> {
  return request<AuditRecord[]>({
    url: `/projects/${encodeURIComponent(id)}/audit`,
    method: 'GET',
    params: query,
  })
}

export function fetchPresence(id: string): Promise<PresenceEntry[]> {
  return request<PresenceEntry[]>({
    url: `/projects/${encodeURIComponent(id)}/presence`,
    method: 'GET',
  })
}

export function pingPresence(id: string, cursorX: number, cursorY: number): Promise<null> {
  return request<null>({
    url: `/projects/${encodeURIComponent(id)}/presence`,
    method: 'POST',
    data: { cursorX, cursorY },
  })
}