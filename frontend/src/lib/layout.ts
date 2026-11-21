import type { GraphEdge, GraphNode } from '@/types'

/** 卡片尺寸，与 AssetNode 的固定宽度一致 */
const NODE_WIDTH = 224
const NODE_HEIGHT = 168
const GAP_X = 96
const GAP_Y = 56
/** 画布左上角留白 */
const ORIGIN_X = 80
const ORIGIN_Y = 80

/**
 * 按依赖关系分层排列：没有入边的节点在第一列，
 * 其余节点的列号取所有前驱的最大列号加一，同层按拓扑序自上而下。
 * 环里的节点退回按访问顺序摆放，不会丢节点。
 */
export function tidyLayout(nodes: GraphNode[], edges: GraphEdge[]): GraphNode[] {
  if (!nodes.length) return nodes

  const ids = new Set(nodes.map((node) => node.id))
  const inbound = new Map<string, string[]>()
  for (const node of nodes) inbound.set(node.id, [])
  for (const edge of edges) {
    if (!ids.has(edge.source) || !ids.has(edge.target)) continue
    inbound.get(edge.target)?.push(edge.source)
  }

  const depth = new Map<string, number>()
  const visiting = new Set<string>()

  /** 递归求层号，遇到环就停在 0，避免死循环 */
  function levelOf(id: string): number {
    const known = depth.get(id)
    if (known !== undefined) return known
    if (visiting.has(id)) return 0
    visiting.add(id)
    const parents = inbound.get(id) ?? []
    const level = parents.length
      ? Math.max(...parents.map((parent) => levelOf(parent) + 1))
      : 0
    visiting.delete(id)
    depth.set(id, level)
    return level
  }

  for (const node of nodes) levelOf(node.id)

  const columns = new Map<number, string[]>()
  for (const node of nodes) {
    const level = depth.get(node.id) ?? 0
    const bucket = columns.get(level) ?? []
    bucket.push(node.id)
    columns.set(level, bucket)
  }

  // 同一列内按原始纵坐标排序，保留作者手工排的相对次序
  const order = new Map(nodes.map((node, index) => [node.id, index]))
  const position = new Map<string, { x: number; y: number }>()
  for (const [level, bucket] of columns) {
    bucket.sort((a, b) => (order.get(a) ?? 0) - (order.get(b) ?? 0))
    bucket.forEach((id, row) => {
      position.set(id, {
        x: ORIGIN_X + level * (NODE_WIDTH + GAP_X),
        y: ORIGIN_Y + row * (NODE_HEIGHT + GAP_Y),
      })
    })
  }

  return nodes.map((node) => ({ ...node, position: position.get(node.id) ?? node.position }))
}