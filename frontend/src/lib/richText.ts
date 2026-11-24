/**
 * 渗透测试详情的富文本处理。
 *
 * 存的是 HTML 片段（contenteditable 产出），三处要用它：
 * 编辑时回填、预览时渲染、导出 Word 时拆成段落与图片。
 * 渲染前一律过一遍白名单清洗，避免粘贴进来的脚本被执行。
 */

/** 允许保留的标签 */
const ALLOWED = new Set(['P', 'DIV', 'BR', 'B', 'STRONG', 'I', 'EM', 'U', 'UL', 'OL', 'LI', 'CODE', 'PRE', 'IMG', 'SPAN'])

/** 允许保留的属性 */
const ALLOWED_ATTRS: Record<string, string[]> = {
  IMG: ['src', 'alt'],
  SPAN: [],
  P: [],
  DIV: [],
  LI: [],
  B: [],
  STRONG: [],
  I: [],
  EM: [],
  U: [],
  UL: [],
  OL: [],
  CODE: [],
  PRE: [],
  BR: [],
}

/** 清洗一段 HTML：只留下白名单标签、安全属性与 data/http(s) 图片 */
export function sanitizeHtml(html: string): string {
  if (!html) return ''
  const doc = new DOMParser().parseFromString(`<div id="root">${html}</div>`, 'text/html')
  const root = doc.getElementById('root')
  if (!root) return ''
  walk(root)
  return root.innerHTML

  function walk(parent: Element): void {
    for (const child of Array.from(parent.children)) {
      if (!ALLOWED.has(child.tagName)) {
        // 不认识的标签拆掉外壳，保留里面的文字
        child.replaceWith(...Array.from(child.childNodes))
        continue
      }
      const keep = ALLOWED_ATTRS[child.tagName] ?? []
      for (const attr of Array.from(child.attributes)) {
        if (!keep.includes(attr.name)) child.removeAttribute(attr.name)
      }
      if (child.tagName === 'IMG') {
        const src = child.getAttribute('src') ?? ''
        // 只拦 javascript:/vbscript: 这类可执行协议；
        // 其他一律放行——本站图片有 /api/pub/uploads、/api/uploads、/uploads 几种历史写法，
        // 逐个前缀去卡很容易把正常图片误删，出现「导出没有图」。
        const risky = /^\s*(javascript|vbscript|file):/i.test(src)
        if (!src.trim() || risky) {
          child.remove()
          continue
        }
      }
      walk(child)
    }
  }
}

/** 富文本里的一个块 */
export interface RichBlock {
  type: 'p' | 'li' | 'img'
  text?: string
  src?: string
  bold?: boolean
}

/** 把 HTML 拆成段落、列表项与图片，导出 Word 用 */
/** 早期的详情可能存的是 Markdown（MCP 直接写入），先把图片语法转成 <img> */
function markdownImagesToHtml(text: string): string {
  if (!text.includes('![')) return text
  return text.replace(/!\[([^\]]*)\]\(([^)\s]+)\)/g, (_all, alt: string, src: string) => {
    return `<img src="${src}" alt="${alt}">`
  })
}

export function htmlToBlocks(html: string): RichBlock[] {
  if (!html) return []
  const prepared = markdownImagesToHtml(html)
  const doc = new DOMParser().parseFromString(`<div id="root">${sanitizeHtml(prepared)}</div>`, 'text/html')
  const root = doc.getElementById('root')
  if (!root) return []

  const blocks: RichBlock[] = []
  for (const node of Array.from(root.childNodes)) {
    collect(node)
  }
  return blocks

  function collect(node: Node): void {
    if (node.nodeType === Node.TEXT_NODE) {
      const text = (node.textContent ?? '').trim()
      if (text) blocks.push({ type: 'p', text })
      return
    }
    if (node.nodeType !== Node.ELEMENT_NODE) return
    const el = node as Element
    if (el.tagName === 'IMG') {
      const src = el.getAttribute('src') ?? ''
      if (src) blocks.push({ type: 'img', src })
      return
    }
    if (el.tagName === 'BR') {
      blocks.push({ type: 'p', text: '' })
      return
    }
    if (el.tagName === 'LI') {
      blocks.push({ type: 'li', text: (el.textContent ?? '').trim() })
      return
    }
    // 其余容器：有块级子元素就继续下钻，否则整体作为一段
    const hasBlocks = Array.from(el.children).some((child) =>
      ['P', 'DIV', 'LI', 'UL', 'OL', 'IMG', 'PRE'].includes(child.tagName),
    )
    if (hasBlocks) {
      for (const child of Array.from(el.childNodes)) collect(child)
      return
    }
    const text = (el.textContent ?? '').trim()
    if (text) blocks.push({ type: 'p', text, bold: el.tagName === 'B' || el.tagName === 'STRONG' })
  }
}

/** 富文本是否为空（只剩空标签也算空） */
export function isBlankHtml(html: string): boolean {
  return htmlToBlocks(html).length === 0
}

/** 纯文本长度，用于校验 */
export function plainLength(html: string): number {
  return htmlToBlocks(html).reduce((sum, block) => sum + (block.text?.length ?? 0), 0)
}