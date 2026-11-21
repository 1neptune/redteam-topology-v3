/** 组件类型（内置 + 自定义） */
export interface AssetType {
  slug: string
  label: string
  color: string
  iconUrl: string | null
  builtin: boolean
  createdBy: string | null
  createdAt: number
  updatedAt: number
}

export interface AssetTypePayload {
  label: string
  color: string
  iconUrl?: string | null
}

/** 上传结果 */
export interface UploadResult {
  id: string
  url: string
  bytes: number
  mime: string
}

/** 健康检查 */
export interface HealthInfo {
  name: string
  version: string
  users: number
  encrypted: boolean
}