import { request } from './http'
import type { CreateUserPayload, UpdateUserPayload, User } from '@/types'

/** 用户列表（管理员） */
export function fetchUsers(): Promise<User[]> {
  return request<User[]>({ url: '/users', method: 'GET' })
}

/** 建号（管理员） */
export function createUser(payload: CreateUserPayload): Promise<User> {
  return request<User>({ url: '/users', method: 'POST', data: payload })
}

/** 改角色 / 启停 / 重置口令（管理员） */
export function updateUser(id: string, payload: UpdateUserPayload): Promise<User> {
  return request<User>({ url: `/users/${encodeURIComponent(id)}`, method: 'PATCH', data: payload })
}
/** 删除账号（管理员） */
export function deleteUser(id: string): Promise<{ deleted: string }> {
  return request<{ deleted: string }>({ url: `/users/${id}`, method: 'DELETE' })
}