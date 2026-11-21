import { request } from './http'
import type { AuditRecord, PageQuery } from '@/types'

/** 全局审计（管理员） */
export function fetchGlobalAudit(query: PageQuery = {}): Promise<AuditRecord[]> {
  return request<AuditRecord[]>({ url: '/audit', method: 'GET', params: query })
}