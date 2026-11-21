/**
 * axios 统一封装：
 * - 自动携带 Bearer 令牌
 * - 解包后端统一响应 { code, msg, data }
 * - 统一抛出中文友好错误，401 时回调登出
 */
import axios, { AxiosError, type AxiosRequestConfig, type AxiosInstance } from 'axios'
import { ApiError, type ApiResponse } from '@/types'
import { clearToken, getToken } from '@/lib/token'

/** 401 回调，由 stores/auth 注册 */
type UnauthorizedHandler = () => void
let onUnauthorized: UnauthorizedHandler | null = null

export function setUnauthorizedHandler(handler: UnauthorizedHandler): void {
  onUnauthorized = handler
}

const instance: AxiosInstance = axios.create({
  baseURL: import.meta.env.VITE_API_BASE || '/api',
  timeout: 20000,
  headers: { 'content-type': 'application/json' },
})

instance.interceptors.request.use((config) => {
  const token = getToken()
  if (token) {
    config.headers = config.headers ?? {}
    config.headers.Authorization = `Bearer ${token}`
  }
  return config
})

instance.interceptors.response.use(
  (response) => response,
  (error: AxiosError<ApiResponse<null>>) => {
    const status = error.response?.status ?? 0
    const payload = error.response?.data
    const message = payload?.msg || fallbackMessage(status, error)

    if (status === 401) {
      clearToken()
      onUnauthorized?.()
    }
    return Promise.reject(new ApiError(status, message))
  },
)

/** 把各状态码映射成中文提示 */
function fallbackMessage(status: number, error: AxiosError): string {
  if (error.code === 'ECONNABORTED') return '请求超时，请稍后重试'
  if (!error.response) return '无法连接服务器，请检查网络或后端服务'
  switch (status) {
    case 400:
      return '请求参数有误'
    case 401:
      return '登录状态已失效，请重新登录'
    case 403:
      return '没有操作权限'
    case 404:
      return '资源不存在'
    case 409:
      return '数据已被其他成员更新，请刷新后重试'
    default:
      return '服务器异常，请稍后重试'
  }
}

/** 统一请求入口：返回解包后的 data */
export async function request<T>(config: AxiosRequestConfig): Promise<T> {
  const response = await instance.request<ApiResponse<T>>(config)
  const body = response.data
  if (!body || typeof body.code !== 'number') {
    throw new ApiError(500, '服务器返回格式异常')
  }
  if (body.code !== 200) {
    if (body.code === 401) {
      clearToken()
      onUnauthorized?.()
    }
    throw new ApiError(body.code, body.msg || '请求失败')
  }
  return body.data as T
}

export { instance as http }