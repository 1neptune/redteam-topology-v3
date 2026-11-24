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

  // 虚线边在快照里会出问题：html-to-image 渲染 stroke-dasharray 时，
  // 每段短线周围会叠出一层比背景更黑的块，连起来像一条黑带。
  // 静态图里虚线也不表达任何含义（那只是「计划中」的视觉提示，
  // 颜色已经能把两类边分开），导出时统一拉成实线。
  const edges = Array.from(el.querySelectorAll<SVGPathElement>('.vue-flow__edge-path'))
  const dashBefore = edges.map((node) => node.style.strokeDasharray)
  // 箭头 marker 用的是 markerUnits="strokeWidth"，尺寸要乘以描边宽度。
  // html-to-image 序列化时拿不到正确的 stroke-width，箭头会被放大十几倍，
  // 变成从节点甩出去的大黑三角。改用用户坐标，尺寸就固定成 markerWidth。
  const markers = Array.from(el.querySelectorAll<SVGMarkerElement>('marker'))
  const unitsBefore = markers.map((node) => node.getAttribute('markerUnits'))

  el.classList.add('is-exporting')
  if (watermark) {
    watermark.style.transition = 'none'
    watermark.style.opacity = '1'
  }

  try {
    await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)))

    for (const node of edges) {
      node.style.strokeDasharray = 'none'
    }
    for (const node of markers) {
      node.setAttribute('markerUnits', 'userSpaceOnUse')
    }

    // 边不再渲染点击热区（LinkEdge 里 interaction-width 设为 0），
    // 热区是 20px 宽的透明描边，html-to-image 会丢掉 stroke-opacity 把它画成黑带。
    // 保留兜底：万一还有残留就清掉。
    for (const node of Array.from(el.querySelectorAll('.vue-flow__edge-interaction'))) {
      node.remove()
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
    })

    const bytes = Math.round((dataUrl.length - dataUrl.indexOf(',') - 1) * 0.75)
    return { dataUrl, width, height, bytes }
  } finally {
    el.classList.remove('is-exporting')
    edges.forEach((node, index) => {
      node.style.strokeDasharray = dashBefore[index] ?? ''
    })
    markers.forEach((node, index) => {
      const previous = unitsBefore[index]
      if (previous == null) node.removeAttribute('markerUnits')
      else node.setAttribute('markerUnits', previous)
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