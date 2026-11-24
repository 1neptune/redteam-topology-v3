/**
 * 漏洞报告导出：Word（.docx）与 PDF。
 *
 * 一个资产可以挂多条漏洞，报告按「汇总表 + 逐条详情」组织。
 * Word 由 docx 库直接构造文档树；PDF 由预览里的只读报告 DOM 截图后按 A4 切片。
 * 两个库都动态引入，只在真正导出时加载，不进首屏包。
 */

import type { RiskLevel, Vulnerability } from '@/types'
import { htmlToBlocks } from '@/lib/richText'
import { imageCache, imageDataUrl } from '@/api/vulnImage'

export type RiskKey = RiskLevel | 'none'

/** 风险等级中文名 */
export const RISK_LABEL: Record<RiskKey, string> = {
  critical: '严重',
  high: '高危',
  medium: '中危',
  low: '低危',
  info: '信息',
  none: '未定级',
}

/** 风险等级排序权重，用于取「最高风险」 */
const RISK_WEIGHT: Record<RiskKey, number> = {
  critical: 5,
  high: 4,
  medium: 3,
  low: 2,
  info: 1,
  none: 0,
}

/** 风险等级对应的颜色变量 */
export const RISK_COLOR_VAR: Record<RiskKey, string> = {
  critical: 'var(--risk-critical)',
  high: 'var(--risk-high)',
  medium: 'var(--risk-medium)',
  low: 'var(--risk-low)',
  info: 'var(--risk-info)',
  none: 'var(--color-muted)',
}

/** 风险等级对应的底色变量 */
export const RISK_SOFT_VAR: Record<RiskKey, string> = {
  critical: 'var(--risk-critical-soft)',
  high: 'var(--risk-high-soft)',
  medium: 'var(--risk-medium-soft)',
  low: 'var(--risk-low-soft)',
  info: 'var(--risk-info-soft)',
  none: 'var(--color-paper-3)',
}

/** 报告里的十六进制风险色（docx 只认 hex） */
const RISK_HEX: Record<RiskKey, string> = {
  critical: 'C0392B',
  high: 'D97706',
  medium: 'A16207',
  low: '2563EB',
  info: '6B7280',
  none: '6B7280',
}

/** 把空串规整为 none */
export function riskKey(risk: string | undefined | null): RiskKey {
  const value = (risk ?? '').trim()
  if (value === 'critical' || value === 'high' || value === 'medium' || value === 'low' || value === 'info') {
    return value
  }
  return 'none'
}

/** 一组漏洞里最高的风险等级 */
export function topRisk(vulns: Vulnerability[]): RiskKey {
  let best: RiskKey = 'none'
  for (const item of vulns) {
    const key = riskKey(item.risk)
    if (RISK_WEIGHT[key] > RISK_WEIGHT[best]) best = key
  }
  return best
}

/** 报告抬头信息 */
export interface ReportMeta {
  projectName: string
  nodeLabel: string
  nodeKindLabel: string
  nodeIp: string
  nodePort: string
  nodeDomain?: string
  author: string
  generatedAt: number
}

/** 时间戳 → 2026-11-20 14:05 */
export function stampText(ms: number): string {
  const date = new Date(ms)
  const pad = (value: number): string => String(value).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`
}

/** 文件名里的安全字符 */
export function reportFileName(meta: ReportMeta, ext: string): string {
  const raw = `${meta.projectName}-${meta.nodeLabel || '资产'}-漏洞报告`
  const safe = raw.replace(/[\\/:*?"<>|\s]+/g, '_').slice(0, 80)
  return `${safe}.${ext}`
}

/** 触发浏览器下载 */
function download(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = filename
  document.body.appendChild(link)
  link.click()
  link.remove()
  setTimeout(() => URL.revokeObjectURL(url), 4000)
}

/** 单条漏洞的展示标题 */
export function vulnTitle(item: Vulnerability, index: number): string {
  const name = item.name.trim()
  return name || `漏洞 ${index + 1}`
}

/** 把 data URL 转成 docx 需要的字节与格式；尺寸按 4:3 兜底（docx 不需要真实比例也能排版） */
function dataUrlBytes(url: string): { data: Uint8Array; type: 'png' | 'jpg'; width: number; height: number } | null {
  try {
    // 预取过就换成 data URL，否则 docx 拿不到二进制
    if (!url.startsWith('data:')) {
      const ready = imageCache.get(url)
      if (ready) url = ready
    }
    const head = url.slice(0, url.indexOf(','))
    const base64 = url.slice(url.indexOf(',') + 1)
    if (!head.includes('base64')) return null
    const mime = head.slice(head.indexOf(':') + 1, head.indexOf(';'))
    const binary = atob(base64)
    const data = new Uint8Array(binary.length)
    for (let i = 0; i < binary.length; i += 1) data[i] = binary.charCodeAt(i)
    return {
      data,
      type: mime.includes('png') ? 'png' : 'jpg',
      width: 800,
      height: 600,
    }
  } catch {
    return null
  }
}

/** 地址加端口；已带端口就不再拼一次，没填地址返回空串 */
function withPort(host?: string, port?: string): string {
  const h = (host ?? '').trim()
  const p = (port ?? '').trim()
  if (!h) return ''
  // 例如 www.example.com:443 或 10.0.0.8:3306 已经带端口，直接返回
  if (/:\d+$/.test(h)) return h
  return p ? `${h}:${p}` : h
}

/** 生成 Word 报告（.docx） */
/**
 * 把正文里引用的图片预取成 data URL，Word 里才能同步嵌进去。
 *
 * 返回取失败的地址：导出时这些图会缺席，界面据此提示，
 * 而不是让人打开 Word 才发现少图。
 */
async function primeImages(vulns: Vulnerability[]): Promise<string[]> {
  const urls = new Set<string>()
  for (const item of vulns) {
    for (const match of (item.detail ?? '').matchAll(/<img[^>]+src="([^"]+)"/g)) {
      const src = match[1]
      if (src && !src.startsWith('data:')) urls.add(src)
    }
  }
  const failed: string[] = []
  await Promise.all(
    [...urls].map(async (url) => {
      try {
        await imageDataUrl(url)
      } catch (err) {
        console.warn('[report] 图片取不回来，Word 里会缺这张', url, err)
        failed.push(url)
      }
    }),
  )
  return failed
}

export async function exportVulnsWord(meta: ReportMeta, vulns: Vulnerability[]): Promise<void> {
  await primeImages(vulns)
  // 取不到的图在 shotParagraph 里会退化成说明文字，不会整个导出失败
  const {
    AlignmentType,
    BorderStyle,
    Document,
    HeadingLevel,
    ImageRun,
    Packer,
    Paragraph,
    Table,
    TableCell,
    TableRow,
    TextRun,
    WidthType,
  } = await import('docx')

  const top = topRisk(vulns)
  const border = { style: BorderStyle.SINGLE, size: 2, color: 'E4E4E7' } as const
  const borders = {
    top: border,
    bottom: border,
    left: border,
    right: border,
    insideHorizontal: border,
    insideVertical: border,
  }

  /** 两列信息行 */
  const metaRow = (key: string, value: string, hex?: string) =>
    new TableRow({
      children: [
        new TableCell({
          width: { size: 22, type: WidthType.PERCENTAGE },
          shading: { fill: 'F4F4F5' },
          children: [new Paragraph({ children: [new TextRun({ text: key, size: 19, color: '52525B' })] })],
        }),
        new TableCell({
          width: { size: 78, type: WidthType.PERCENTAGE },
          children: [new Paragraph({ children: [new TextRun({ text: value || '—', size: 19, color: hex })] })],
        }),
      ],
    })

  /** 富文本块：段落、列表项与图片，图片按版心 460pt 等比缩放 */
  const richBlocks = (html: string, index: number): import('docx').Paragraph[] => {
    const blocks = htmlToBlocks(html)
    if (!blocks.length) {
      return [
        new Paragraph({
          spacing: { before: 200, after: 80 },
          children: [new TextRun({ text: '渗透测试详情', size: 20, bold: true, color: '3F3F46' })],
        }),
        new Paragraph({ children: [new TextRun({ text: '（未填写）', size: 21 })] }),
      ]
    }
    const out: import('docx').Paragraph[] = [
      new Paragraph({
        spacing: { before: 200, after: 80 },
        children: [new TextRun({ text: '渗透测试详情', size: 20, bold: true, color: '3F3F46' })],
      }),
    ]
    for (const item of blocks) {
      if (item.type === 'img' && item.src) {
        out.push(shotParagraph(item.src, index))
        continue
      }
      out.push(
        new Paragraph({
          spacing: { after: 40, line: 320 },
          bullet: item.type === 'li' ? { level: 0 } : undefined,
          children: [new TextRun({ text: item.text ?? '', size: 21, bold: item.bold })],
        }),
      )
    }
    return out
  }

  /** 一张内联图片的段落；读不出来就退化成一行说明 */
  const shotParagraph = (src: string, index: number): import('docx').Paragraph => {
    const parsed = dataUrlBytes(src)
    if (!parsed) {
      return new Paragraph({
        children: [new TextRun({ text: `（漏洞 ${index + 1} 的图片无法解析）`, size: 17, color: 'A1A1AA' })],
      })
    }
    return new Paragraph({
      spacing: { before: 120, after: 120 },
      children: [
        new ImageRun({
          type: parsed.type,
          data: parsed.data,
          transformation: { width: 460, height: Math.round(460 * (parsed.height / parsed.width)) },
        }),
      ],
    })
  }

  /** 段落块：小标题 + 逐行正文 */
  const block = (heading: string, body: string) => {
    const text = body.trim()
    const lines = text ? text.split(/\r?\n/) : ['（未填写）']
    return [
      new Paragraph({
        spacing: { before: 200, after: 80 },
        children: [new TextRun({ text: heading, size: 20, bold: true, color: '3F3F46' })],
      }),
      ...lines.map(
        (line) =>
          new Paragraph({
            spacing: { after: 40, line: 320 },
            children: [new TextRun({ text: line, size: 21 })],
          }),
      ),
    ]
  }

  /** 每条漏洞一节 */
  const sections = vulns.flatMap((item, index) => {
    const risk = riskKey(item.risk)
    const head = [
      new Paragraph({
        heading: HeadingLevel.HEADING_1,
        spacing: { before: 300, after: 60 },
        children: [
          new TextRun({ text: `${index + 1}. ${vulnTitle(item, index)}`, size: 26, bold: true }),
          new TextRun({ text: `    ${RISK_LABEL[risk]}`, size: 20, bold: true, color: RISK_HEX[risk] }),
        ],
      }),
    ]
    return [
      ...head,
      ...block('漏洞说明', item.description),
      ...richBlocks(item.detail, index),
      ...block('处置建议', item.remediation),
    ]
  })

  const summaryRows = vulns.map(
    (item, index) =>
      new TableRow({
        children: [
          new TableCell({
            width: { size: 8, type: WidthType.PERCENTAGE },
            children: [
              new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: String(index + 1), size: 19 })] }),
            ],
          }),
          new TableCell({
            width: { size: 68, type: WidthType.PERCENTAGE },
            children: [new Paragraph({ children: [new TextRun({ text: vulnTitle(item, index), size: 19 })] })],
          }),
          new TableCell({
            width: { size: 24, type: WidthType.PERCENTAGE },
            children: [
              new Paragraph({
                children: [
                  new TextRun({ text: RISK_LABEL[riskKey(item.risk)], size: 19, color: RISK_HEX[riskKey(item.risk)] }),
                ],
              }),
            ],
          }),
        ],
      }),
  )

  const doc = new Document({
    creator: meta.author,
    title: `${meta.nodeLabel || '资产'} 渗透测试漏洞报告`,
    description: `${meta.projectName} / ${meta.nodeLabel}`,
    sections: [
      {
        properties: { page: { margin: { top: 1000, bottom: 1000, left: 1100, right: 1100 } } },
        children: [
          new Paragraph({
            spacing: { after: 60 },
            children: [new TextRun({ text: '渗透测试漏洞报告', size: 36, bold: true })],
          }),
          new Paragraph({
            spacing: { after: 240 },
            children: [
              new TextRun({ text: `${meta.projectName} / ${meta.nodeLabel}`, size: 19, color: '71717A' }),
            ],
          }),
          new Table({
            width: { size: 100, type: WidthType.PERCENTAGE },
            borders,
            rows: [
              metaRow('资产节点', `${meta.nodeLabel}（${meta.nodeKindLabel || '未分类'}）`),
              ...(withPort(meta.nodeIp, meta.nodePort)
      ? [metaRow('IP / 主机端口信息', withPort(meta.nodeIp, meta.nodePort))]
      : []),
    ...(withPort(meta.nodeDomain, meta.nodePort)
      ? [metaRow('域名端口信息', withPort(meta.nodeDomain, meta.nodePort))]
      : []),
              metaRow('漏洞数量', `${vulns.length} 条`),
              metaRow('最高风险', RISK_LABEL[top], RISK_HEX[top]),
              metaRow('报告生成', `${stampText(meta.generatedAt)}  ${meta.author || '未知'}`),
            ],
          }),
          ...(vulns.length
            ? [
                new Paragraph({
                  spacing: { before: 320, after: 120 },
                  children: [new TextRun({ text: '漏洞汇总', size: 24, bold: true })],
                }),
                new Table({
                  width: { size: 100, type: WidthType.PERCENTAGE },
                  borders,
                  rows: [metaRow('序号', ''), ...summaryRows].slice(1),
                }),
              ]
            : []),
          ...sections,
          new Paragraph({
            spacing: { before: 360 },
            alignment: AlignmentType.LEFT,
            children: [
              new TextRun({
                text: '本报告由 RedTeam Atlas 生成，内容以项目内记录为准。',
                size: 17,
                color: 'A1A1AA',
              }),
            ],
          }),
        ],
      },
    ],
  })

  const blob = await Packer.toBlob(doc)
  download(blob, reportFileName(meta, 'docx'))
}

/**
 * 由只读报告 DOM 生成 PDF（A4 竖版，按页高切片）。
 *
 * 用 html-to-image 而不是 html2canvas：前者把节点序列化成 SVG foreignObject
 * 交给浏览器自己排版，能解析 CSS 变量与 oklch()；html2canvas 自己解析样式，
 * 遇到 oklch 会直接抛错。
 */
export async function exportVulnsPdf(sheet: HTMLElement, filename: string): Promise<void> {
  const [{ toCanvas }, { jsPDF }] = await Promise.all([import('html-to-image'), import('jspdf')])

  const canvas = await toCanvas(sheet, {
    pixelRatio: 2,
    backgroundColor: '#ffffff',
    width: 794,
    height: sheet.scrollHeight,
    style: { left: '0px', top: '0px', margin: '0px' },
  })

  const pdf = new jsPDF({ unit: 'mm', format: 'a4', orientation: 'portrait' })
  const pageWidth = 210
  const pageHeight = 297
  const imageHeight = (canvas.height * pageWidth) / canvas.width
  const data = canvas.toDataURL('image/jpeg', 0.94)

  let heightLeft = imageHeight
  let position = 0
  pdf.addImage(data, 'JPEG', 0, position, pageWidth, imageHeight)
  heightLeft -= pageHeight

  while (heightLeft > 0) {
    position -= pageHeight
    pdf.addPage()
    pdf.addImage(data, 'JPEG', 0, position, pageWidth, imageHeight)
    heightLeft -= pageHeight
  }

  pdf.save(filename)
}
