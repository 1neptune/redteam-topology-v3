import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import * as authApi from '@/api/auth'
import { setUnauthorizedHandler } from '@/api/http'
import { clearToken, getToken, setToken } from '@/lib/token'
import type { DirectoryEntry, UserRole } from '@/types'

/** 当前会话用户（/auth/me 的返回） */
export interface SessionUser {
  id: string
  username: string
  displayName: string
  role: UserRole
}

/** 登录态、当前用户与成员名录 */
export const useAuthStore = defineStore('auth', () => {
  const token = ref(getToken())
  const user = ref<SessionUser | null>(null)
  const directory = ref<DirectoryEntry[]>([])
  const ready = ref(false)

  const isAuthenticated = computed(() => Boolean(token.value))
  const isAdmin = computed(() => user.value?.role === 'admin')
  const displayName = computed(() => user.value?.displayName ?? '')

  let signOutHandler: (() => void) | null = null

  /** 注册 401 回调，供路由跳转使用 */
  function onSignOut(handler: () => void): void {
    signOutHandler = handler
  }

  // 令牌失效时统一清理并通知外层
  setUnauthorizedHandler(() => {
    token.value = ''
    user.value = null
    signOutHandler?.()
  })

  /** 启动时恢复会话：有令牌就拉一次当前用户 */
  async function bootstrap(): Promise<void> {
    if (ready.value) return
    if (!token.value) {
      ready.value = true
      return
    }
    try {
      user.value = await authApi.fetchMe()
    } catch {
      clearToken()
      token.value = ''
      user.value = null
    } finally {
      ready.value = true
    }
  }

  /** 登录 */
  async function login(username: string, password: string): Promise<void> {
    const result = await authApi.login({ username, password })
    setToken(result.token)
    token.value = result.token
    user.value = {
      id: result.user.id,
      username: result.user.username,
      displayName: result.user.displayName,
      role: result.user.role,
    }
    ready.value = true
  }

  /** 登出：清本地令牌即可，JWT 为无状态 */
  async function logout(): Promise<void> {
    try {
      await authApi.logout()
    } catch {
      /* 令牌可能已失效，忽略 */
    }
    clearToken()
    token.value = ''
    user.value = null
  }

  /** 修改口令：服务端会签发新令牌，旧令牌立即失效 */
  async function changePassword(current: string, next: string): Promise<void> {
    const result = await authApi.changePassword({ current, next })
    setToken(result.token)
    token.value = result.token
  }

  /** 刷新成员名录（授权选择器用） */
  async function refreshDirectory(): Promise<void> {
    directory.value = await authApi.fetchDirectory()
  }

  return {
    token,
    user,
    directory,
    ready,
    isAuthenticated,
    isAdmin,
    displayName,
    onSignOut,
    bootstrap,
    login,
    logout,
    changePassword,
    refreshDirectory,
  }
})