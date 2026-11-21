import type { AssetType, EdgeData, EdgeStatus, NodeData, NodeStatus } from '@/types'

/** 内置组件的图标（SVG path，视图盒 24×24） */
export const BUILTIN_ICONS: Record<string, string> = {
  attacker: 'M12 4.5a2.5 2.5 0 1 1 0 5a2.5 2.5 0 0 1 0-5M4.5 20v-1.5A4.5 4.5 0 0 1 9 14h6a4.5 4.5 0 0 1 4.5 4.5V20',
  entry: 'M4 12h12l-4-4M16 12l-4 4M20 4v16',
  web: 'M4 5h16v14H4zM4 9h16M7 7h.01M10 7h.01',
  cloud: 'M7 18h10a4 4 0 0 0 0-8 6 6 0 0 0-11.6 1.5A3.5 3.5 0 0 0 7 18z',
  host: 'M4 4h16v6H4zM4 14h16v6H4zM8 7h.01M8 17h.01',
  domain: 'M12 3l8 4v6c0 4-3.5 6.5-8 8-4.5-1.5-8-4-8-8V7z',
  database: 'M12 4c4 0 7 1 7 2.5S16 9 12 9 5 8 5 6.5 8 4 12 4zM5 6.5v11C5 19 8 20 12 20s7-1 7-2.5v-11',
  workstation: 'M4 5h16v10H4zM9 20h6M12 15v5',
  network: 'M12 4v6M6 20v-6M18 20v-6M6 14h12M12 10v4',
  firewall: 'M4 6h16v12H4zM4 10h16M4 14h16M9 6v4M15 10v4M9 14v4',
  pivot: 'M4 12h6l3-4 3 8 4-4M4 12h.01',
  credential: 'M14 4a5 5 0 1 0 2.5 9.3L20 17v3h-3l-1.5-1.5',
  vuln: 'M12 3l9 17H3zM12 9v5M12 17h.01',
  flag: 'M6 4v16M6 5h11l-2 4 2 4H6',
}

/** 节点控制状态 */
export const NODE_STATUSES: Array<{ value: NodeStatus; label: string; tone: string }> = [
  { value: 'recon', label: '侦察中', tone: 'system' },
  { value: 'attack', label: '攻击中', tone: 'bad' },
  { value: 'control', label: '已控制', tone: 'ok' },
  { value: 'blocked', label: '受阻中', tone: 'warn' },
]

/** 链路状态 */
export const EDGE_STATUSES: Array<{ value: EdgeStatus; label: string; tone: string }> = [
  { value: 'planned', label: '计划中', tone: '' },
  { value: 'active', label: '进行中', tone: 'system' },
  { value: 'success', label: '已成功', tone: 'ok' },
  { value: 'failed', label: '失败', tone: 'bad' },
]

/** 常用攻击手法（输入建议，不限制自定义） */
export const TECHNIQUES: string[] = [
  'SSRF',
  'SQL 注入',
  '文件上传 GetShell',
  '弱口令爆破',
  '钓鱼投递',
  'NTLM Relay',
  'Pass-the-Hash',
  'Kerberoasting',
  'C2 回连',
  '数据外带',
  '权限提升',
]

/** 组件库分组 */
const GROUP_OF: Record<string, string> = {
  attacker: '入口 / 暴露面',
  entry: '入口 / 暴露面',
  web: '入口 / 暴露面',
  cloud: '入口 / 暴露面',
  host: '内网资产',
  domain: '内网资产',
  database: '内网资产',
  workstation: '内网资产',
  network: '网络 / 边界',
  firewall: '网络 / 边界',
  pivot: '网络 / 边界',
  credential: '情报 / 战果',
  vuln: '情报 / 战果',
  flag: '情报 / 战果',
  note: '情报 / 战果',
}

/** 解析节点类型（找不到时回落到 host 外观） */
export function resolveType(kind: string, types: AssetType[]): AssetType {
  const found = types.find((t) => t.slug === kind)
  if (found) return found
  const fallback = types.find((t) => t.slug === 'host')
  if (fallback) return fallback
  return {
    slug: kind || 'host',
    label: '资产',
    color: '#9fb3c8',
    iconUrl: null,
    builtin: true,
    createdBy: null,
    createdAt: 0,
    updatedAt: 0,
  }
}

/** 组件图标 path */
export function iconOf(slug: string): string {
  return BUILTIN_ICONS[slug] ?? BUILTIN_ICONS.host
}

/** 按分组整理组件 */
export function groupTypes(types: AssetType[]): Array<{ group: string; items: AssetType[] }> {
  const map = new Map<string, AssetType[]>()
  for (const type of types) {
    const group = type.builtin ? GROUP_OF[type.slug] ?? '其它' : '自定义'
    const list = map.get(group) ?? []
    list.push(type)
    map.set(group, list)
  }
  return [...map.entries()].map(([group, items]) => ({ group, items }))
}

/** 节点状态元信息 */
export function statusMeta(status: NodeStatus | string): { label: string; tone: string } {
  const found = NODE_STATUSES.find((s) => s.value === status)
  // 老拓扑里可能还留着 unknown / vuln / attacking / owned，统一退到「侦察」
  return found ?? NODE_STATUSES[0]
}

/** 链路状态元信息 */
export function edgeStatusMeta(status: EdgeStatus | string): { label: string; tone: string } {
  const found = EDGE_STATUSES.find((s) => s.value === status)
  return found ? { label: found.label, tone: found.tone } : { label: '计划中', tone: '' }
}

/** 生成节点数据默认值 */
export function makeNodeData(kind: string, label = ''): NodeData {
  return {
    label,
    kind,
    ip: '',
    port: '',
    domain: '',
    os: '',
    status: 'recon',
    notes: '',
    fields: [],
    vulns: [],
  }
}

/** 生成链路数据默认值 */
export function makeEdgeData(): EdgeData {
  return {
    status: 'planned',
    notes: '',
    fields: [],
    animated: false,
  }
}
