/**
 * 画布导出：把当前画布元素截图成 PNG。
 * 导出瞬间给容器加 is-exporting 类，隐藏控件与交互热区。
 */
import { toPng } from 'html-to-image'

/** 渲染结果：data URL 交给预览显示 */
export interface ExportImage {
  dataUrl: string
  width: number
  height: number
  bytes: number
}

/** 落盘后的结果 */
export interface ExportResult {
  width: number
  height: number
  bytes: number
}

/** 触发浏览器下载 */
export function downloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  document.body.appendChild(a)
  a.click()
  a.remove()
  setTimeout(() => URL.revokeObjectURL(url), 4000)
}

/**
 * 把画布渲染成 data URL。
 *
 * 渲染期间临时清掉选择态与控件：边的点击热区宽 20px，离屏时会退化成黑粗线；
 * 控件与缩略图不该出现在成品里。
 */
async function capture(el: HTMLElement | null, pixelRatio: number): Promise<ExportImage> {
  if (!el) throw new Error('画布还未准备好')

  const watermark = el.querySelector<HTMLElement>('[data-export-watermark]')
  const interactions = Array.from(el.querySelectorAll<SVGPathElement>('.vue-flow__edge-interaction'))
  const previousWidths = interactions.map((node) => node.getAttribute('stroke-width'))

  el.classList.add('is-exporting')
  if (watermark) {
    watermark.style.transition = 'none'
    watermark.style.opacity = '1'
  }

  try {
    await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)))
    for (const node of interactions) {
      node.setAttribute('stroke-width', '0')
      node.style.strokeWidth = '0'
    }

    const rect = el.getBoundingClientRect()
    const width = Math.round(rect.width * pixelRatio)
    const height = Math.round(rect.height * pixelRatio)
    const dataUrl = await toPng(el, {
      backgroundColor: getComputedStyle(el).backgroundColor || undefined,
      width: Math.round(rect.width),
      height: Math.round(rect.height),
      pixelRatio,
      skipFonts: true,
      cacheBust: true,
      filter: (node) => {
        const target = node as HTMLElement | SVGElement | null
        const cls = target?.classList
        if (!cls) return true
        return !(
          cls.contains('vue-flow__edge-interaction') ||
          cls.contains('vue-flow__controls') ||
          cls.contains('vue-flow__minimap') ||
          cls.contains('vue-flow__attribution')
        )
      },
    })

    const bytes = Math.round((dataUrl.length - dataUrl.indexOf(',') - 1) * 0.75)
    return { dataUrl, width, height, bytes }
  } finally {
    el.classList.remove('is-exporting')
    interactions.forEach((node, index) => {
      const previous = previousWidths[index]
      if (previous == null) node.removeAttribute('stroke-width')
      else node.setAttribute('stroke-width', previous)
      node.style.strokeWidth = ''
    })
    if (watermark) {
      watermark.style.opacity = '0'
      watermark.style.transition = ''
    }
  }
}

/** 只渲染不下载，供预览使用 */
export function renderCanvasPng(el: HTMLElement | null, pixelRatio = 2): Promise<ExportImage> {
  return capture(el, pixelRatio)
}

/** 渲染并直接下载 */
export async function exportCanvasPng(
  el: HTMLElement | null,
  filename: string,
  pixelRatio = 2,
): Promise<ExportResult> {
  const shot = await capture(el, pixelRatio)
  const blob = await (await fetch(shot.dataUrl)).blob()
  downloadBlob(blob, filename)
  return { width: shot.width, height: shot.height, bytes: blob.size }
}

/** 把 data URL 存成文件 */
export async function saveImage(dataUrl: string, filename: string): Promise<number> {
  const blob = await (await fetch(dataUrl)).blob()
  downloadBlob(blob, filename)
  return blob.size
}