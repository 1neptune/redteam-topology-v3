import { defineStore } from 'pinia'
import { tidyLayout } from '@/lib/layout'
import { computed, ref } from 'vue'
import * as projectApi from '@/api/project'
import { ApiError } from '@/types'
import type {
  AuditRecord,
  GraphEdge,
  GraphNode,
  GraphViewport,
  MemberEntry,
  NodeData,
  Permission,
  PresenceEntry,
  ProjectSummary,
  Vulnerability,
} from '@/types'
import { makeEdgeData, makeNodeData } from '@/lib/nodeTypes'
import { newId } from '@/lib/ids'
import { useAuthStore } from './auth'

const AUTOSAVE_DELAY = 1200
const PRESENCE_PING_MS = 5000
const PRESENCE_POLL_MS = 6500

/** 画布编辑：拓扑、选中、保存、审计、在线成员 */
export const useEditorStore = defineStore('editor', () => {
  const auth = useAuthStore()
  const project = ref<ProjectSummary | null>(null)
  const nodes = ref<GraphNode[]>([])
  const edges = ref<GraphEdge[]>([])
  const viewport = ref<GraphViewport>({ x: 0, y: 0, zoom: 1 })
  const permission = ref<Permission>('viewer')

  const loading = ref(false)
  const error = ref('')
  const saving = ref(false)
  const dirty = ref(false)
  const lastSavedAt = ref(0)
  const conflict = ref<{ version: number } | null>(null)

  const audit = ref<AuditRecord[]>([])
const auditLoading = ref(false)
  const members = ref<MemberEntry[]>([])
  const presence = ref<PresenceEntry[]>([])

  const selectedNodeId = ref('')
  const selectedEdgeId = ref('')

  let autosaveTimer = 0
  let pingTimer = 0
  let pollTimer = 0

  const canEdit = computed(() => permission.value === 'owner' || permission.value === 'editor')
  const canManage = computed(() => permission.value === 'owner')

  const selectedNode = computed<GraphNode | null>(
    () => nodes.value.find((n) => n.id === selectedNodeId.value) ?? null,
  )
  const selectedEdge = computed<GraphEdge | null>(
    () => edges.value.find((e) => e.id === selectedEdgeId.value) ?? null,
  )

  const stats = computed(() => {
    const byStatus: Record<string, number> = {}
    for (const node of nodes.value) {
      byStatus[node.data.status] = (byStatus[node.data.status] ?? 0) + 1
    }
    return { nodes: nodes.value.length, edges: edges.value.length, byStatus }
  })

  /** 某个节点/连线的最后编辑者 */
  function lastEditorOf(targetId: string): { name: string; at: number } | null {
    const hit = audit.value.find((record) => record.targetId === targetId)
    if (!hit) return null
    return { name: hit.userName, at: hit.createdAt }
  }

  /** 载入项目 */
  async function load(projectId: string): Promise<void> {
    loading.value = true
    error.value = ''
    dirty.value = false
    conflict.value = null
    try {
      const detail = await projectApi.fetchProject(projectId)
      project.value = detail.project
      permission.value = detail.permission
      nodes.value = detail.graph.nodes.map((node) => ({
        ...node,
        type: 'asset',
        data: { ...makeNodeData(node.data.kind), ...node.data },
      }))
      edges.value = detail.graph.edges.map((edge) => ({
        ...edge,
        type: 'link',
        markerEnd: edge.markerEnd || 'arrowclosed',
        data: { ...makeEdgeData(), ...edge.data },
      }))
      viewport.value = detail.graph.viewport
      lastSavedAt.value = detail.project.updatedAt
      selectedNodeId.value = ''
      selectedEdgeId.value = ''
      await Promise.all([loadAudit(true), loadMembers()])
      startPresence()
    } catch (err) {
      error.value = err instanceof Error ? err.message : '项目加载失败'
    } finally {
      loading.value = false
    }
  }

  /** 撤销栈：每次改动前压入一份快照，最多保留 50 步 */
  const history = ref<Array<{ nodes: string; edges: string }>>([])

  /** 清空撤销栈（切项目时调用） */
  function clearHistory(): void {
    history.value = []
  }

  /** 改动前留一份快照 */
  function snapshot(): void {
    if (!canEdit.value) return
    history.value.push({ nodes: JSON.stringify(nodes.value), edges: JSON.stringify(edges.value) })
    if (history.value.length > 50) history.value.shift()
  }

  /** 撤销上一步；返回是否真的撤销了 */
  function undo(): boolean {
    if (!canEdit.value) return false
    const last = history.value.pop()
    if (!last) return false
    try {
      nodes.value = JSON.parse(last.nodes) as GraphNode[]
      edges.value = JSON.parse(last.edges) as GraphEdge[]
    } catch {
      return false
    }
    selectedNodeId.value = ''
    selectedEdgeId.value = ''
    touch()
    return true
  }

  /** 是否还有可撤销的步骤 */
  const canUndo = computed(() => history.value.length > 0)

  /** 一键整理：按依赖分层重排节点坐标，并让画布适配内容 */
  function tidy(): void {
    if (!canEdit.value || !nodes.value.length) return
    snapshot()
    nodes.value = tidyLayout(nodes.value, edges.value)
    touch()
  }

  /** 标记有未保存改动，并触发防抖自动保存 */
  function touch(): void {
    if (!canEdit.value) return
    dirty.value = true
    window.clearTimeout(autosaveTimer)
    autosaveTimer = window.setTimeout(() => {
      void save({ silent: true })
    }, AUTOSAVE_DELAY)
  }

  /** 保存拓扑（版本号乐观锁） */
  async function save(options: { silent?: boolean } = {}): Promise<boolean> {
    if (!project.value || !canEdit.value) return false
    if (saving.value) return false
    if (!dirty.value) return true

    saving.value = true
    try {
      const result = await projectApi.saveProject(project.value.id, {
        data: { nodes: nodes.value, edges: edges.value, viewport: viewport.value },
        baseVersion: project.value.version,
      })
      project.value = {
        ...project.value,
        version: result.version,
        nodeCount: result.nodeCount,
        edgeCount: result.edgeCount,
        updatedAt: result.updatedAt,
        updatedBy: '我',
      }
      dirty.value = false
      conflict.value = null
      lastSavedAt.value = result.updatedAt
      await loadAudit(true)
      return true
    } catch (err) {
      if (err instanceof ApiError && err.code === 409) {
        conflict.value = { version: project.value.version + 1 }
      } else if (!options.silent) {
        error.value = err instanceof Error ? err.message : '保存失败'
      }
      return false
    } finally {
      saving.value = false
    }
  }

  /** 冲突时用本地覆盖：重新拉版本号后再保存 */
  async function forceSave(): Promise<boolean> {
    const current = project.value
    if (!current) return false
    try {
      const detail = await projectApi.fetchProject(current.id)
      project.value = { ...current, version: detail.project.version }
      dirty.value = true
      return await save({ silent: false })
    } catch {
      return false
    }
  }

  /** 冲突时放弃本地改动，载入服务器版本 */
  async function reloadLatest(): Promise<void> {
    if (!project.value) return
    await load(project.value.id)
  }

  /** 为新节点找一个不与现有节点重叠的位置 */
  function findFreeSpot(base: { x: number; y: number }): { x: number; y: number } {
    const width = 232
    const height = 104
    let candidate = { ...base }
    for (let attempt = 0; attempt < 12; attempt += 1) {
      const clash = nodes.value.some(
        (node) =>
          Math.abs(node.position.x - candidate.x) < width &&
          Math.abs(node.position.y - candidate.y) < height,
      )
      if (!clash) return candidate
      candidate = {
        x: candidate.x + width,
        y: attempt >= 3 ? candidate.y + height : candidate.y,
      }
    }
    return candidate
  }

  /** 新建节点 */
  function createNode(kind: string, position: { x: number; y: number }, label = ''): GraphNode | null {
    if (!canEdit.value) return null
    snapshot()
    const spot = findFreeSpot(position)
    const node: GraphNode = {
      id: newId('n_'),
      type: 'asset',
      position: { x: Math.round(spot.x), y: Math.round(spot.y) },
      data: { ...makeNodeData(kind), label },
    }
    nodes.value = [...nodes.value, node]
    selectNode(node.id)
    touch()
    return node
  }

  /** 删除节点及其连线 */
  function removeNode(nodeId: string): void {
    if (!canEdit.value) return
    snapshot()
    nodes.value = nodes.value.filter((n) => n.id !== nodeId)
    edges.value = edges.value.filter((e) => e.source !== nodeId && e.target !== nodeId)
    if (selectedNodeId.value === nodeId) selectedNodeId.value = ''
    touch()
  }

  /** 复制节点，偏移 32px */
  function duplicateNode(nodeId: string): GraphNode | null {
    if (!canEdit.value) return null
    snapshot()
    const source = nodes.value.find((n) => n.id === nodeId)
    if (!source) return null
    const copy: GraphNode = {
      ...source,
      id: newId('n_'),
      position: { x: source.position.x + 32, y: source.position.y + 32 },
      data: JSON.parse(JSON.stringify(source.data)) as NodeData,
      selected: false,
    }
    nodes.value = [...nodes.value, copy]
    selectNode(copy.id)
    touch()
    return copy
  }

  /** 建立链路 */
  function addEdge(source: string, target: string): { ok: boolean; reason?: string } {
    if (!canEdit.value) return { ok: false, reason: 'readonly' }
    if (!source || !target || source === target) return { ok: false, reason: 'invalid' }
    const exists = edges.value.some((e) => e.source === source && e.target === target)
    if (exists) return { ok: false, reason: 'duplicate' }

    const edge: GraphEdge = {
      id: newId('e_'),
      source,
      target,
      label: '',
      type: 'link',
      markerEnd: 'arrowclosed',
      data: makeEdgeData(),
    }
    edges.value = [...edges.value, edge]
    selectEdge(edge.id)
    touch()
    return { ok: true }
  }

  function removeEdge(edgeId: string): void {
    if (!canEdit.value) return
    snapshot()
    edges.value = edges.value.filter((e) => e.id !== edgeId)
    if (selectedEdgeId.value === edgeId) selectedEdgeId.value = ''
    touch()
  }

  /** 反转链路方向 */
  function reverseEdge(edgeId: string): void {
    if (!canEdit.value) return
    snapshot()
    edges.value = edges.value.map((edge) =>
      edge.id === edgeId ? { ...edge, source: edge.target, target: edge.source } : edge,
    )
    touch()
  }

  /** 更新节点字段 */
  function updateNode(nodeId: string, patch: Partial<NodeData>): void {
    if (!canEdit.value) return
    nodes.value = nodes.value.map((node) =>
      node.id === nodeId ? { ...node, data: { ...node.data, ...patch } } : node,
    )
    touch()
  }

  /** 写入节点漏洞列表（双击节点后保存整组） */
  function updateVulnerabilities(nodeId: string, vulns: Vulnerability[]): void {
    if (!canEdit.value) return
    const stamp = { updatedAt: Date.now(), updatedBy: auth.displayName }
    const next = vulns.map((item) => ({ ...item, ...stamp }))
    nodes.value = nodes.value.map((node) =>
      node.id === nodeId ? { ...node, data: { ...node.data, vulns: next } } : node,
    )
    touch()
  }

  /** 更新连线字段 */
  function updateEdge(
    edgeId: string,
    patch: { label?: string; animated?: boolean; data?: Partial<GraphEdge['data']> },
  ): void {
    if (!canEdit.value) return
    edges.value = edges.value.map((edge) => {
      if (edge.id !== edgeId) return edge
      const next: GraphEdge = { ...edge }
      if (patch.label !== undefined) next.label = patch.label
      if (patch.animated !== undefined) next.data = { ...edge.data, animated: patch.animated }
      if (patch.data) next.data = { ...edge.data, ...patch.data }
      return next
    })
    touch()
  }

  function selectNode(nodeId: string): void {
    selectedNodeId.value = nodeId
    selectedEdgeId.value = ''
  }

  function selectEdge(edgeId: string): void {
    selectedEdgeId.value = edgeId
    selectedNodeId.value = ''
  }

  function clearSelection(): void {
    selectedNodeId.value = ''
    selectedEdgeId.value = ''
  }

  /** 拉取审计（replace=false 时追加更早记录） */
  async function loadAudit(replace = true): Promise<void> {
    if (!project.value || auditLoading.value) return
    auditLoading.value = true
    try {
      const before = replace ? undefined : audit.value[audit.value.length - 1]?.createdAt
      const rows = await projectApi.fetchAudit(project.value.id, { limit: 40, before })
      audit.value = replace ? rows : [...audit.value, ...rows]
    } finally {
      auditLoading.value = false
    }
  }

  async function loadMembers(): Promise<void> {
    if (!project.value) return
    members.value = await projectApi.fetchMembers(project.value.id)
  }

  /** 开始在线心跳与光标同步 */
  function startPresence(): void {
    stopPresence()
    pingTimer = window.setInterval(() => {
      if (!project.value) return
      void projectApi.pingPresence(project.value.id, 0, 0).catch(() => undefined)
    }, PRESENCE_PING_MS)
    pollTimer = window.setInterval(() => {
      if (!project.value) return
      void projectApi
        .fetchPresence(project.value.id)
        .then((list) => {
          presence.value = list
        })
        .catch(() => undefined)
    }, PRESENCE_POLL_MS)
    const current = project.value
    if (current) void projectApi.pingPresence(current.id, 0, 0).catch(() => undefined)
  }

  function stopPresence(): void {
    window.clearInterval(pingTimer)
    window.clearInterval(pollTimer)
    pingTimer = 0
    pollTimer = 0
  }

  /** 离开编辑器时重置全部状态 */
  function reset(): void {
    window.clearTimeout(autosaveTimer)
    stopPresence()
    project.value = null
    nodes.value = []
    edges.value = []
    viewport.value = { x: 0, y: 0, zoom: 1 }
    permission.value = 'viewer'
    audit.value = []
    members.value = []
    presence.value = []
    selectedNodeId.value = ''
    selectedEdgeId.value = ''
    loading.value = false
    error.value = ''
    saving.value = false
    dirty.value = false
    conflict.value = null
  }

  /** 记录他人保存的版本（用于提示） */
  function noteRemoteSave(version: number): void {
    if (!project.value) return
    if (version > project.value.version) {
      project.value = { ...project.value, version }
      conflict.value = { version }
    }
  }

  return {
    project,
    nodes,
    edges,
    viewport,
    permission,
    loading,
    error,
    saving,
    dirty,
    lastSavedAt,
    conflict,
    audit,
    auditLoading,
    members,
    presence,
    selectedNodeId,
    selectedEdgeId,
    canEdit,
    canManage,
    selectedNode,
    selectedEdge,
    stats,
    lastEditorOf,
    load,
    save,
    forceSave,
    reloadLatest,
    touch,
    undo,
    canUndo,
    clearHistory,
    tidy,
    createNode,
    removeNode,
    duplicateNode,
    addEdge,
    removeEdge,
    reverseEdge,
    updateNode,
    updateVulnerabilities,
    updateEdge,
    selectNode,
    selectEdge,
    clearSelection,
    loadAudit,
    loadMembers,
    startPresence,
    stopPresence,
    reset,
    noteRemoteSave,
  }
})