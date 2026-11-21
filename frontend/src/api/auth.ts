import { request } from './http'
import type { ChangePasswordPayload, DirectoryEntry, LoginPayload, LoginResult } from '@/types'

/** 登录（系统唯一公开接口） */
export function login(payload: LoginPayload): Promise<LoginResult> {
  return request<LoginResult>({ url: '/auth/login', method: 'POST', data: payload })
}

export function logout(): Promise<null> {
  return request<null>({ url: '/auth/logout', method: 'POST' })
}

export function fetchMe(): Promise<{
  id: string
  username: string
  displayName: string
  role: 'admin' | 'member'
}> {
  return request({ url: '/auth/me', method: 'GET' })
}

export function changePassword(payload: ChangePasswordPayload): Promise<LoginResult> {
  return request<LoginResult>({ url: '/auth/password', method: 'POST', data: payload })
}

/** 成员名录（授权选择器） */
export function fetchDirectory(): Promise<DirectoryEntry[]> {
  return request<DirectoryEntry[]>({ url: '/auth/users', method: 'GET' })
}