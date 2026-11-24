/** 漏洞配图：上传、取回、转 data URL。 */

import { http } from './http'
import { uploadIcon } from './asset'

/** 单张图片上限（与后端 uploads 限制一致） */
export const IMAGE_MAX_BYTES = 8 * 1024 * 1024

/** 一条漏洞最多挂几张图 */
export const IMAGE_MAX_COUNT = 8

/** upload id 或签名地址 → data URL 的会话内缓存 */
export const imageCache = new Map<string, string>()
const cache = imageCache

/**
 * upload id 或签名地址 → 图片真实像素尺寸。
 *
 * 上传时 compress 已经解码过图片，顺手记下来；导出 Word 时直接取，
 * 不必再从文件头解析——那一步在压缩流上容易读错。
 */
export const imageSizeCache = new Map<string, { width: number; height: number }>()

/** 编辑框内联图片的最长边，超过就等比缩小 */
const INLINE_MAX_EDGE = 1600

/** 压到 JPEG 画布上，返回新的 data URL 与原始像素尺寸（同尺寸小图直接返回原样） */
async function compress(file: File): Promise<{ dataUrl: string; width: number; height: number }> {
  const url = URL.createObjectURL(file)
  try {
    const image = await new Promise<HTMLImageElement>((resolve, reject) => {
      const probe = new Image()
      probe.onload = () => resolve(probe)
      probe.onerror = () => reject(new Error('图片无法解码'))
      probe.src = url
    })
    const size = { width: image.naturalWidth, height: image.naturalHeight }
    const scale = Math.min(1, INLINE_MAX_EDGE / Math.max(image.naturalWidth, image.naturalHeight))
    // 已经足够小就不要再压一遍，避免无谓损失
    if (scale === 1 && file.size <= 320 * 1024 && file.type !== 'image/png') {
      const raw = await toBase64(file)
      return { dataUrl: `data:${file.type};base64,${raw}`, ...size }
    }
    const canvas = document.createElement('canvas')
    canvas.width = Math.max(1, Math.round(image.naturalWidth * scale))
    canvas.height = Math.max(1, Math.round(image.naturalHeight * scale))
    const ctx = canvas.getContext('2d')
    if (!ctx) throw new Error('浏览器不支持画布')
    ctx.drawImage(image, 0, 0, canvas.width, canvas.height)
    // 返回画布尺寸而不是原始尺寸：导出时的显示比例要跟压缩后的图一致
    return { dataUrl: canvas.toDataURL('image/jpeg', 0.82), width: canvas.width, height: canvas.height }
  } finally {
    URL.revokeObjectURL(url)
  }
}

/** 文件转 base64（不带 data: 前缀） */
function toBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => {
      const result = String(reader.result ?? '')
      resolve(result.slice(result.indexOf(',') + 1))
    }
    reader.onerror = () => reject(new Error('读取文件失败'))
    reader.readAsDataURL(file)
  })
}

/**
 * 上传一张图片，返回可直接放进正文的短地址。
 *
 * 图片作为独立资源存服务端，正文只留带签名的 URL：
 * 十几年前的老报告也一样能加载，拓扑 JSON 体积不受图片大小影响，
 * 也不会再出现被字符上限截断的问题。
 *
 * @param file 浏览器 File
 */
export async function uploadImage(file: File): Promise<string> {
  if (!file.type.startsWith('image/')) {
    throw new Error('只能上传图片')
  }
  if (file.size > IMAGE_MAX_BYTES) {
    throw new Error('图片不能超过 8MB')
  }
  const { dataUrl, width, height } = await compress(file)
  const base64 = dataUrl.slice(dataUrl.indexOf(',') + 1)
  const result = await uploadIcon(file.name, 'image/jpeg', base64)
  // 缓存 data URL 与尺寸，导出 Word 时不必再取、也不必再解析
  cache.set(result.id, dataUrl)
  cache.set(result.url, dataUrl)
  imageSizeCache.set(result.id, { width, height })
  imageSizeCache.set(result.url, { width, height })
  return result.url
}

/**
 * 取图片的 data URL。
 *
 * 报告导出要嵌进 Word、还要被 html-to-image 序列化，
 * blob URL 在这两处都不可靠，因此统一转成 data URL。
 */
export async function imageDataUrl(idOrUrl: string): Promise<string> {
  const cached = cache.get(idOrUrl)
  if (cached) return cached
  // 正文里存的是 /api/pub/uploads/... 这样的完整路径，而 http 的 baseURL 已经是 /api，
  // 直接传会被拼成 /api/api/... 打不开；这类地址用 baseURL='' 原样请求。
  const isFullPath = idOrUrl.startsWith('/') || idOrUrl.startsWith('http')
  const path = isFullPath ? idOrUrl : `/uploads/${idOrUrl}`
  const response = await http.get(path, isFullPath ? { responseType: 'blob', baseURL: '' } : { responseType: 'blob' })
  const blob = response.data as Blob
  const dataUrl = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(String(reader.result ?? ''))
    reader.onerror = () => reject(new Error('读取图片失败'))
    reader.readAsDataURL(blob)
  })
  cache.set(idOrUrl, dataUrl)
  return dataUrl
}