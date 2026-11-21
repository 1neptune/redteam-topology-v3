import { request } from './http'
import type { GithubRelease, UpgradeResult, VersionInfo } from '@/types'

/** 本机版本与仓库信息 */
export function fetchVersion(): Promise<VersionInfo> {
  return request<VersionInfo>({ url: '/system/version', method: 'GET' })
}

/**
 * 查询官网最新发布版本。
 * 直连 GitHub API（其响应带 CORS 头），拿不到时返回 null，
 * 由调用方回落到本地 update.json 的离线清单。
 */
export async function fetchLatestRelease(checkUrl: string): Promise<GithubRelease | null> {
  if (!checkUrl) return null
  try {
    const response = await fetch(checkUrl, {
      headers: { Accept: 'application/vnd.github+json' },
      cache: 'no-store',
    })
    if (!response.ok) return null
    const data = (await response.json()) as Record<string, unknown>
    const tagName = typeof data.tag_name === 'string' ? data.tag_name : ''
    if (!tagName) return null
    return {
      tagName,
      name: typeof data.name === 'string' ? data.name : tagName,
      body: typeof data.body === 'string' ? data.body : '',
      htmlUrl: typeof data.html_url === 'string' ? data.html_url : '',
      publishedAt: typeof data.published_at === 'string' ? data.published_at : '',
    }
  } catch {
    return null
  }
}

/** 版本号逐段比较：a 是否比 b 新 */
export function isNewerVersion(a: string, b: string): boolean {
  const parse = (raw: string): number[] =>
    raw
      .trim()
      .replace(/^v/i, '')
      .split(/[.\-+]/)
      .map((part) => Number.parseInt(part, 10) || 0)

  const left = parse(a)
  const right = parse(b)
  const length = Math.max(left.length, right.length)
  for (let index = 0; index < length; index += 1) {
    const l = left[index] ?? 0
    const r = right[index] ?? 0
    if (l !== r) return l > r
  }
  return false
}
/**
 * 一键更新：下载官网最新二进制并让本机重启。
 * 成功后进程会在约 1.5 秒内退出，由替换脚本拉起新版本。
 */
export function runUpgrade(): Promise<UpgradeResult> {
  return request<UpgradeResult>({ url: '/system/upgrade', method: 'POST' })
}
/** 运行日志的一行（按后端返回的 columns 顺序） */
export interface LogPayload {
  kind: 'access' | 'error'
  columns: string[]
  rows: string[][]
}

/** 读取运行日志（仅管理员） */
export function fetchLogs(kind: 'access' | 'error', limit = 300): Promise<LogPayload> {
  return request<LogPayload>({ url: '/system/logs', method: 'GET', params: { kind, limit } })
}