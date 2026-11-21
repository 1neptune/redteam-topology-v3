import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import * as assetApi from '@/api/asset'
import type { AssetType, AssetTypePayload } from '@/types'

const MIME_BY_EXT: Record<string, string> = {
  png: 'image/png',
  jpg: 'image/jpeg',
  jpeg: 'image/jpeg',
  webp: 'image/webp',
  gif: 'image/gif',
  svg: 'image/svg+xml',
}

/** 允许上传的图片类型（与服务端白名单一致） */
export const ALLOWED_ICON_MIMES = Object.values(MIME_BY_EXT)

/** 组件库：内置 + 自定义组件，含图标对象 URL 缓存 */
export const useCatalogStore = defineStore('catalog', () => {
  const types = ref<AssetType[]>([])
  const loading = ref(false)
  const loaded = ref(false)
  /** iconUrl → 对象 URL（图标接口需要登录，不能直接作为 img src） */
  const iconMap = ref<Record<string, string>>({})

  const customTypes = computed(() => types.value.filter((t) => !t.builtin))

  /** 拉取组件清单（只拉一次，除非强制刷新） */
  async function load(force = false): Promise<void> {
    if (loaded.value && !force) return
    loading.value = true
    try {
      types.value = await assetApi.fetchAssetTypes()
      loaded.value = true
      await primeIcons()
    } finally {
      loading.value = false
    }
  }

  /** 预取所有自定义组件的图标 */
  async function primeIcons(): Promise<void> {
    const pending = types.value
      .map((type) => type.iconUrl)
      .filter((url): url is string => Boolean(url) && !iconMap.value[url as string])
    await Promise.all(
      pending.map(async (url) => {
        try {
          iconMap.value = { ...iconMap.value, [url]: await assetApi.fetchIconBlob(url) }
        } catch {
          /* 图标取不到时回退为内置字形 */
        }
      }),
    )
  }

  /** 取图标地址（未加载完成时返回空串，组件回退到内置图标） */
  function iconSrc(iconUrl: string | null | undefined): string {
    if (!iconUrl) return ''
    return iconMap.value[iconUrl] ?? ''
  }

  /** 新建自定义组件 */
  async function create(payload: AssetTypePayload): Promise<AssetType> {
    const created = await assetApi.createAssetType(payload)
    types.value = [...types.value, created]
    await primeIcons()
    return created
  }

  /** 修改自定义组件 */
  async function update(slug: string, payload: AssetTypePayload): Promise<AssetType> {
    const updated = await assetApi.updateAssetType(slug, payload)
    types.value = types.value.map((t) => (t.slug === slug ? updated : t))
    await primeIcons()
    return updated
  }

  /** 删除自定义组件 */
  async function remove(slug: string): Promise<void> {
    await assetApi.deleteAssetType(slug)
    types.value = types.value.filter((t) => t.slug !== slug)
  }

  /** 上传图标文件，返回可直接使用的服务端地址 */
  async function uploadIcon(file: File): Promise<string> {
    const ext = file.name.split('.').pop()?.toLowerCase() ?? ''
    const mime = file.type || MIME_BY_EXT[ext] || ''
    if (!ALLOWED_ICON_MIMES.includes(mime)) {
      throw new Error('只支持 PNG / JPEG / WebP / GIF / SVG 图片')
    }
    if (file.size > 2 * 1024 * 1024) {
      throw new Error('图标大小不能超过 2MB')
    }
    const base64 = await readAsBase64(file)
    const result = await assetApi.uploadIcon(file.name, mime, base64)
    return result.url
  }

  return {
    types,
    loading,
    loaded,
    iconMap,
    customTypes,
    load,
    primeIcons,
    iconSrc,
    create,
    update,
    remove,
    uploadIcon,
  }
})

/** 读取文件为纯 base64（去掉 dataURL 前缀） */
function readAsBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => {
      const result = String(reader.result ?? '')
      const index = result.indexOf(',')
      resolve(index >= 0 ? result.slice(index + 1) : result)
    }
    reader.onerror = () => reject(new Error('读取文件失败'))
    reader.readAsDataURL(file)
  })
}