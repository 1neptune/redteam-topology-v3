import { http, request } from './http'
import type { AssetType, AssetTypePayload, UploadResult } from '@/types'

/** 组件类型清单 */
export function fetchAssetTypes(): Promise<AssetType[]> {
  return request<AssetType[]>({ url: '/asset-types', method: 'GET' })
}

export function createAssetType(payload: AssetTypePayload): Promise<AssetType> {
  return request<AssetType>({ url: '/asset-types', method: 'POST', data: payload })
}

export function updateAssetType(slug: string, payload: AssetTypePayload): Promise<AssetType> {
  return request<AssetType>({
    url: `/asset-types/${encodeURIComponent(slug)}`,
    method: 'PATCH',
    data: payload,
  })
}

export function deleteAssetType(slug: string): Promise<null> {
  return request<null>({ url: `/asset-types/${encodeURIComponent(slug)}`, method: 'DELETE' })
}

/** 上传图标（base64，服务端做类型与大小校验） */
export function uploadIcon(filename: string, mime: string, dataBase64: string): Promise<UploadResult> {
  return request<UploadResult>({
    url: '/uploads',
    method: 'POST',
    data: { filename, mime, dataBase64 },
  })
}

/**
 * 取得图标二进制并转为对象 URL。
 * 图标接口需要登录，浏览器无法在 <img src> 上带 Authorization 头，
 * 因此由前端带令牌取流再交给 img。
 */
export async function fetchIconBlob(iconUrl: string): Promise<string> {
  const path = iconUrl.replace(/^\/api/, '')
  const response = await http.get(path, { responseType: 'blob' })
  return URL.createObjectURL(response.data as Blob)
}