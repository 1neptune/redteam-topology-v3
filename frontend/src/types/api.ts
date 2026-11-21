/** 统一响应包装，与后端 `{ code, msg, data }` 一致 */
export interface ApiResponse<T> {
  code: number
  msg: string
  data: T | null
}

/** 业务错误（由 axios 拦截器抛出，message 已是中文友好文案） */
export class ApiError extends Error {
  readonly code: number

  constructor(code: number, message: string) {
    super(message)
    this.name = 'ApiError'
    this.code = code
  }
}

/** 分页查询通用参数 */
export interface PageQuery {
  limit?: number
  before?: number
}