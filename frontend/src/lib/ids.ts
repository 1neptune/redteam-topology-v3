/** 生成带前缀的随机 ID，与后端命名风格一致 */
export function newId(prefix: string): string {
  const bytes = new Uint8Array(8)
  crypto.getRandomValues(bytes)
  const hex = Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('')
  return `${prefix}${hex}`
}
/** 漏洞条目 ID：`v_` + 32 位十六进制，与后端校验规则一致 */
export function newVulnId(): string {
  const bytes = new Uint8Array(16)
  crypto.getRandomValues(bytes)
  const hex = Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('')
  return `v_${hex}`
}