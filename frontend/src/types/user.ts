export type UserRole = 'admin' | 'member'

/** 账号（不含口令） */
export interface User {
  id: string
  username: string
  displayName: string
  role: UserRole
  disabled: boolean
  createdAt: number
  lastLoginAt: number | null
}

/** 登录/注册返回 */
export interface LoginResult {
  token: string
  expiresAt: number
  user: User
}

/** 登录页公开信息 */
export interface AuthState {
  allowRegistration: boolean
  userCount: number
}

/** 授权选择器用的成员名录 */
export interface DirectoryEntry {
  id: string
  username: string
  displayName: string
}

export interface LoginPayload {
  username: string
  password: string
}

export interface RegisterPayload extends LoginPayload {
  displayName?: string
}

export interface ChangePasswordPayload {
  current: string
  next: string
}

export interface CreateUserPayload {
  username: string
  password: string
  displayName?: string
  role?: UserRole
  /** 建号后直接授权的项目 */
  projectId?: string
  /** 授权级别：owner / editor / viewer */
  permission?: string
}

export interface UpdateUserPayload {
  displayName?: string
  role?: UserRole
  disabled?: boolean
  password?: string
}