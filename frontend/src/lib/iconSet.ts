/**
 * 自定义组件的 5 个内置图标。
 *
 * 图标地址存两种值：
 * - `builtin:<key>`：内置图标，前端按 key 渲染 SVG，不占存储
 * - `/api/uploads/<id>`：用户上传的图片
 *
 * 新建组件时随机预选一个内置图标，用户可换或上传替换。
 */

export interface BuiltinIcon {
  key: BuiltinIconKey
  label: string
  /** 24×24 画布的路径数据，统一描边渲染 */
  path: string
}

export type BuiltinIconKey = 'target' | 'bug' | 'server' | 'cloud' | 'key'

export const BUILTIN_ICONS: BuiltinIcon[] = [
  {
    key: 'target',
    label: '靶标',
    path: 'M12 3a9 9 0 1 0 0 18a9 9 0 0 0 0-18M12 8a4 4 0 1 0 0 8a4 4 0 0 0 0-8M12 1.5v3M12 19.5v3M1.5 12h3M19.5 12h3',
  },
  {
    key: 'bug',
    label: '漏洞',
    path: 'M8 8h8v5a4 4 0 0 1-8 0zM12 8V5.5a2 2 0 0 1 2-2M6 9H3M6 13H3M18 9h3M18 13h3M9 18l-1.4 2.2M15 18l1.4 2.2',
  },
  {
    key: 'server',
    label: '服务器',
    path: 'M3 4.5h18v6H3zM3 13.5h18v6H3zM6.5 7.5h0.01M6.5 16.5h0.01',
  },
  {
    key: 'cloud',
    label: '云资源',
    path: 'M7.5 18.5a4.5 4.5 0 0 1 0-9 5.5 5.5 0 0 1 10.6 1.6 3.7 3.7 0 0 1-0.6 7.4z',
  },
  {
    key: 'key',
    label: '凭证',
    path: 'M14.5 6.5a3.5 3.5 0 1 1 0 7a3.5 3.5 0 0 1 0-7M11.9 11.4L3.5 19.8M5.6 17.7l2.1 2.1M7.9 15.4l2.1 2.1',
  },
]

const BY_KEY = new Map(BUILTIN_ICONS.map((icon) => [icon.key, icon]))

/** 图标地址是否为内置图标 */
export function isBuiltinIcon(iconUrl: string | null | undefined): boolean {
  return typeof iconUrl === 'string' && iconUrl.startsWith('builtin:')
}

/** 按图标地址取内置图标定义；非内置返回 null */
export function builtinOf(iconUrl: string | null | undefined): BuiltinIcon | null {
  if (!isBuiltinIcon(iconUrl)) return null
  const key = (iconUrl as string).slice('builtin:'.length)
  return BY_KEY.get(key as BuiltinIconKey) ?? null
}

/** 随机取一个内置图标地址，用于新建组件时的预选 */
export function randomBuiltinUrl(): string {
  const index = Math.floor(Math.random() * BUILTIN_ICONS.length)
  return `builtin:${BUILTIN_ICONS[index].key}`
}

/** 由 key 拼出图标地址 */
export function builtinUrl(key: BuiltinIconKey): string {
  return `builtin:${key}`
}