/** 时间与字符串格式化工具 */

function pad(value: number): string {
  return value < 10 ? `0${value}` : String(value)
}

/** 完整时间：2026-11-19 16:30 */
export function formatDateTime(ms: number): string {
  if (!ms) return '—'
  const d = new Date(ms)
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
}

/** 只取时分 */
export function formatClock(ms: number): string {
  if (!ms) return '—'
  const d = new Date(ms)
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`
}

/** 相对时间：刚刚 / 12 分钟前 / 3 天前 */
export function formatRelative(ms: number): string {
  if (!ms) return '—'
  const diff = Date.now() - ms
  if (diff < 45_000) return '刚刚'
  const minutes = Math.floor(diff / 60_000)
  if (minutes < 60) return `${minutes} 分钟前`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours} 小时前`
  const days = Math.floor(hours / 24)
  if (days < 30) return `${days} 天前`
  return formatDateTime(ms)
}

/** 文件名安全化（保留中文） */
export function safeFilename(name: string | undefined, fallback = 'topology'): string {
  const cleaned = (name ?? '')
    .replace(/[\\/:*?"<>|\u0000-\u001f]/g, '')
    .replace(/\s+/g, '-')
    .slice(0, 60)
  return cleaned || fallback
}

/** 导出用的时间戳：20261119-1630 */
export function fileStamp(): string {
  const d = new Date()
  return `${d.getFullYear()}${pad(d.getMonth() + 1)}${pad(d.getDate())}-${pad(d.getHours())}${pad(d.getMinutes())}`
}

/** 截断文本并补省略号 */
export function ellipsis(text: string, max: number): string {
  const value = text ?? ''
  if (value.length <= max) return value
  return `${value.slice(0, Math.max(0, max - 1))}…`
}
/** 年月日时分，用户管理表格用 */
export function formatStamp(ms?: number | null): string {
  if (!ms) return '从未登录'
  const date = new Date(ms)
  const pad = (value: number): string => String(value).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`
}