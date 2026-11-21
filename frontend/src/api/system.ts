import { request } from './http'
import type { VersionInfo } from '@/types'

/** 本机版本与仓库地址（只读，不做联网检查） */
export function fetchVersion(): Promise<VersionInfo> {
  return request<VersionInfo>({ url: '/system/version', method: 'GET' })
}

/** 运行日志内容 */
export interface LogPayload {
  kind: string
  columns: string[]
  rows: string[][]
}

/** 读运行日志：kind = access | error */
export function fetchLogs(kind: 'access' | 'error', limit = 200): Promise<LogPayload> {
  return request<LogPayload>({
    url: '/system/logs',
    method: 'GET',
    params: { kind, limit },
  })
}