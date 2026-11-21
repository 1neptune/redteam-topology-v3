/** JWT 令牌的本地存取（与 axios 解耦，避免循环依赖） */

const TOKEN_KEY = 'atlas.token'

export function getToken(): string {
  try {
    return localStorage.getItem(TOKEN_KEY) ?? ''
  } catch {
    return ''
  }
}

export function setToken(token: string): void {
  try {
    if (token) localStorage.setItem(TOKEN_KEY, token)
    else localStorage.removeItem(TOKEN_KEY)
  } catch {
    /* 隐私模式下忽略 */
  }
}

export function clearToken(): void {
  setToken('')
}