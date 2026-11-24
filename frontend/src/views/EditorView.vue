<script setup lang="ts">
import { computed, markRaw, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { storeToRefs } from 'pinia'
import { VueFlow, useVueFlow, ConnectionMode } from '@vue-flow/core'
import type {
  EdgeComponent,
  EdgeMouseEvent,
  EdgeTypesObject,
  NodeComponent,
  NodeMouseEvent,
  NodeTypesObject,
} from '@vue-flow/core'
import { Background } from '@vue-flow/background'
import { Controls } from '@vue-flow/controls'
import { MiniMap } from '@vue-flow/minimap'

import AssetNode from '@/components/AssetNode.vue'
import LinkEdge from '@/components/LinkEdge.vue'
import PalettePanel from '@/components/PalettePanel.vue'
import InspectorPanel from '@/components/InspectorPanel.vue'
import ContextMenu, { type MenuItem } from '@/components/ContextMenu.vue'
import AssetTypeDialog from '@/components/AssetTypeDialog.vue'
import MemberDialog from '@/components/MemberDialog.vue'
import VulnPanel from '@/components/VulnPanel.vue'
import PngPreviewDialog from '@/components/PngPreviewDialog.vue'
import ReportPreview from '@/components/ReportPreview.vue'
import ChangePasswordDialog from '@/components/ChangePasswordDialog.vue'

import { useEditorStore } from '@/stores/editor'
import { useCatalogStore } from '@/stores/catalog'
import { useAuthStore } from '@/stores/auth'
import { useUiStore } from '@/stores/ui'
import { exportCanvasPng, downloadBlob, renderCanvasPng, saveImage, type ExportImage } from '@/lib/exporter'
import { bestScore } from '@/lib/fuzzy'
import { fileStamp, formatClock, safeFilename } from '@/lib/format'
import { resolveType } from '@/lib/nodeTypes'
import * as projectApi from '@/api/project'
import type { GraphNode } from '@/types'

const route = useRoute()
const router = useRouter()
const editor = useEditorStore()
const catalog = useCatalogStore()
const auth = useAuthStore()
const ui = useUiStore()
const { nodes, edges } = storeToRefs(editor)

const flowId = 'atlas-flow'
const { screenToFlowCoordinate, fitView } = useVueFlow(flowId)

const nodeTypes = { asset: markRaw(AssetNode) as unknown as NodeComponent } as NodeTypesObject
const edgeTypes = { link: markRaw(LinkEdge) as unknown as EdgeComponent } as EdgeTypesObject

const wrapper = ref<HTMLElement | null>(null)
const showTypes = ref(false)
const showMembers = ref(false)
const showVuln = ref(false)
const vulnNode = ref<GraphNode | null>(null)
const previewVulns = ref<Parameters<typeof editor.updateVulnerabilities>[1]>([])
const showPreview = ref(false)
const showPng = ref(false)
const pngBusy = ref(false)
const pngShot = ref<ExportImage | null>(null)
const showPassword = ref(false)
const previewAt = ref(Date.now())
const menuNodeId = ref('')
const menuEdgeId = ref('')

/** 拖出来的连线松手在空白处时，记录起点节点，弹出目标组件列表 */
const linkSource = ref('')
const pendingFrom = ref('')
const connectSucceeded = ref(false)

const menu = reactive<{
  visible: boolean
  x: number
  y: number
  title: string
  items: MenuItem[]
  flowPos: { x: number; y: number } | null
}>({ visible: false, x: 0, y: 0, title: '', items: [], flowPos: null })

const projectId = computed(() => String(route.params.id ?? ''))
const isEmpty = computed(() => !editor.loading && !editor.error && editor.nodes.length === 0)

const PERMISSION_LABEL: Record<string, string> = { owner: '项目管理', editor: '可编辑', viewer: '只读' }

const saveState = computed(() => {
  if (editor.conflict) return { text: '版本冲突', tone: 'bad' }
  if (editor.saving) return { text: '保存中', tone: '' }
  if (editor.dirty) return { text: '未保存', tone: 'warn' }
  return { text: `已保存 ${formatClock(editor.lastSavedAt)}`, tone: 'ok' }
})

const remotePresence = computed(() =>
  editor.presence.filter((item) => item.userId !== auth.user?.id),
)

/** 以组件默认名新建资产 */
function createAsset(slug: string, position: { x: number; y: number }): void {
  const label = resolveType(slug, catalog.types).label
  editor.createNode(slug, position, label)
}

/** 拖拽连线完成 */
function onConnect(params: { source: string; target: string }): void {
  connectSucceeded.value = true
  linkSource.value = ''
  const result = editor.addEdge(params.source, params.target)
  if (!result.ok && result.reason === 'duplicate') ui.toast('两个节点之间已存在链路', 'warn')
}

/** 开始拉线：记下起点并清掉上一次的成功标记 */
function onConnectStart(payload: { nodeId?: string | null }): void {
  pendingFrom.value = payload.nodeId ?? ''
  connectSucceeded.value = false
}

/**
 * 连线松手：落到节点上由 onConnect 处理；
 * 落在空白处则把其余资产组件列成菜单，点一个即建立链路。
 */
function onConnectEnd(event?: MouseEvent): void {
  const from = pendingFrom.value
  pendingFrom.value = ''
  if (connectSucceeded.value) {
    connectSucceeded.value = false
    return
  }
  if (!from) return

  const targets = editor.nodes.filter((node) => node.id !== from)
  if (!targets.length) {
    ui.toast('还没有其他组件可以连接', 'warn')
    return
  }
  linkSource.value = from
  const items = targets.map((node) => ({
    id: `to-${node.id}`,
    label: node.data.label || '未命名组件',
    icon: ICON.copy,
  }))
  if (event) {
    openMenu(event, items, '连接到')
  }
}

/**
 * 节点双击：打开渗透测试漏洞详情面板。
 * 与单击严格分开——单击只做选中与右侧属性编辑，双击才进入漏洞详情。
 * 双击前浏览器会先派发两次 click，因此此处再选中一次，保证右侧属性同步。
 */
function onNodeDoubleClick(payload: { node: GraphNode }): void {
  editor.selectNode(payload.node.id)
  vulnNode.value = payload.node
  showVuln.value = true
}

/** 保存漏洞列表（写回节点 data.vulns，触发自动保存） */
function onSaveVuln(vulns: Parameters<typeof editor.updateVulnerabilities>[1]): void {
  const target = vulnNode.value
  if (!target) return
  editor.updateVulnerabilities(target.id, vulns)
  ui.toast('漏洞记录已保存', 'ok')
}

/** 撤销一步，给顶栏按钮与 Ctrl+Z 共用 */
function undo(): void {
  if (!editor.canEdit) return
  if (!editor.undo()) {
    ui.toast('没有可撤销的操作')
  }
}

/** 快捷键：Ctrl+Z 撤销、Ctrl+S 保存、Delete 删除选中元素 */
function onHotkey(event: KeyboardEvent): void {
  const tag = (event.target as HTMLElement | null)?.tagName ?? ''
  const typing = tag === 'INPUT' || tag === 'TEXTAREA' || (event.target as HTMLElement | null)?.isContentEditable
  const meta = event.ctrlKey || event.metaKey

  if (meta && event.key.toLowerCase() === 'z') {
    event.preventDefault()
    undo()
    return
  }
  if (meta && event.key.toLowerCase() === 's') {
    event.preventDefault()
    void editor.save().then((ok) => ui.toast(ok ? '已保存' : '没有需要保存的改动', ok ? 'ok' : undefined))
    return
  }
  // V：打开当前选中节点的漏洞详情，等同双击节点
  if (!typing && !meta && event.key.toLowerCase() === 'v') {
    const node = editor.nodes.find((n) => n.id === editor.selectedNodeId)
    if (!node) return
    event.preventDefault()
    onNodeDoubleClick({ node })
    return
  }
  if (!typing && (event.key === 'Delete' || event.key === 'Backspace')) {
    if (!editor.selectedNodeId && !editor.selectedEdgeId) return
    event.preventDefault()
    void removeSelected()
  }
}

/** 删除当前选中元素（快捷键与面板共用） */
async function removeSelected(): Promise<void> {
  if (!editor.canEdit) return
  if (editor.selectedNodeId) {
    const name = editor.selectedNode?.data.label || '未命名'
    const ok = await ui.confirm({ title: '删除节点', message: `将删除「${name}」及其关联链路。`, confirmText: '删除', danger: true })
    if (ok) editor.removeNode(editor.selectedNodeId)
    return
  }
  if (editor.selectedEdgeId) {
    editor.removeEdge(editor.selectedEdgeId)
  }
}

/** 打开报告预览（按当前草稿渲染，不必先保存） */
function onPreviewVulns(vulns: Parameters<typeof editor.updateVulnerabilities>[1]): void {
  if (!vulnNode.value) return
  previewVulns.value = vulns
  previewAt.value = Date.now()
  showPreview.value = true
}

/** 报告抬头 */
const reportMeta = computed(() => ({
  projectName: editor.project?.name ?? '',
  nodeLabel: vulnNode.value?.data.label ?? '',
  nodeKindLabel: vulnNode.value ? resolveType(vulnNode.value.data.kind, catalog.types).label : '',
  nodeIp: vulnNode.value?.data.ip ?? '',
  nodePort: vulnNode.value?.data.port ?? '',
  nodeDomain: vulnNode.value?.data.domain ?? '',
  author: auth.displayName,
  generatedAt: previewAt.value,
}))

/** 节点单击：选中并在右侧属性面板编辑 */
function onNodeClick(payload: { node: GraphNode }): void {
  editor.selectNode(payload.node.id)
}

function onEdgeClick(payload: { edge: { id: string } }): void {
  editor.selectEdge(payload.edge.id)
}

function onPaneClick(): void {
  editor.clearSelection()
}

function onDragOver(event: DragEvent): void {
  event.preventDefault()
  if (event.dataTransfer) event.dataTransfer.dropEffect = 'copy'
}

/** 从组件库拖入画布 */
function onDrop(event: DragEvent): void {
  event.preventDefault()
  const slug = event.dataTransfer?.getData('application/atlas-node')
  if (!slug) return
  if (!editor.canEdit) {
    ui.toast('当前为只读权限', 'warn')
    return
  }
  try {
    const pos = screenToFlowCoordinate({ x: event.clientX, y: event.clientY })
    createAsset(slug, { x: pos.x - 106, y: pos.y - 42 })
  } catch {
    createAsset(slug, { x: 220, y: 180 })
  }
}

/** 双击空白处快速新建主机 */
/** 节点搜索：输入后模糊匹配节点名，选中即定位 */
const searchKeyword = ref('')
const searchOpen = ref(false)
const searchIndex = ref(0)
const searchInput = ref<HTMLInputElement | null>(null)

/** 定位后回填关键字，此时不要再把下拉弹出来 */
let suppressSearchOpen = false

const searchHits = computed(() => {
  const kw = searchKeyword.value.trim()
  if (!kw) return []

  return editor.nodes
    .map((node) => ({
      node,
      score: bestScore(kw, [node.data.label || '', node.data.ip || '', node.data.domain || '']),
    }))
    .filter((row): row is { node: (typeof editor.nodes)[number]; score: number } => row.score !== null)
    .sort((a, b) => a.score - b.score)
    .slice(0, 8)
})

/**
 * 定位到一个节点：选中它，并把视野聚到它身上。
 *
 * 选中写在节点自身的 selected 上——Vue Flow 认这个字段，
 * store 里的 selectedNodeId 只驱动右侧属性面板，两者都要给。
 */
async function focusNode(node: (typeof editor.nodes)[number]): Promise<void> {
  editor.selectNode(node.id)
  for (const item of nodes.value) {
    item.selected = item.id === node.id
  }
  searchOpen.value = false
  suppressSearchOpen = true
  searchKeyword.value = node.data.label || ''
  await nextTick()
  // 只聚焦这一个节点；节点高度随内容变，让它自己算中心
  await fitView({ nodes: [node.id], padding: 0.8, duration: 260, maxZoom: 1.2 })
}

/** 回车取第一条，上下键在结果里移动 */
function onSearchKey(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    searchOpen.value = false
    searchKeyword.value = ''
    return
  }
  if (!searchHits.value.length) return
  if (event.key === 'ArrowDown') {
    event.preventDefault()
    searchIndex.value = (searchIndex.value + 1) % searchHits.value.length
    return
  }
  if (event.key === 'ArrowUp') {
    event.preventDefault()
    searchIndex.value = (searchIndex.value - 1 + searchHits.value.length) % searchHits.value.length
    return
  }
  if (event.key === 'Enter') {
    event.preventDefault()
    const hit = searchHits.value[searchIndex.value] ?? searchHits.value[0]
    if (hit) void focusNode(hit.node)
  }
}

/** 输入变化时重置高亮项 */
watch(searchKeyword, () => {
  searchIndex.value = 0
  if (suppressSearchOpen) {
    suppressSearchOpen = false
    return
  }
  searchOpen.value = searchKeyword.value.trim().length > 0
})
function onCanvasDoubleClick(event: MouseEvent): void {
  if (!editor.canEdit) return
  const target = event.target as HTMLElement | null
  if (target?.closest('.vue-flow__node') || target?.closest('.vue-flow__edge')) return
  try {
    const pos = screenToFlowCoordinate({ x: event.clientX, y: event.clientY })
    createAsset('host', { x: pos.x - 106, y: pos.y - 42 })
  } catch {
    createAsset('host', { x: 220, y: 180 })
  }
}

/** 适应视图 */
function doFit(): void {
  try {
    fitView({ padding: 0.2, duration: 200, maxZoom: 1.1 })
  } catch {
    /* 画布未就绪 */
  }
}

/* 右键菜单 */

const ICON = {
  copy: 'M9 9h10v12H9zM5 3h10v4M5 3v12h4',
  trash: 'M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13',
  reverse: 'M8 7h9l-3-3M16 17H7l3 3',
  fit: 'M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5',
  image: 'M4 16l4-5 3 3 3-4 6 6M4 5h16v14H4z',
  plus: 'M12 5v14M5 12h14',
}

/** 打开菜单 */
function openMenu(event: MouseEvent, items: MenuItem[], title = ''): void {
  menu.x = event.clientX
  menu.y = event.clientY
  menu.items = items
  menu.title = title
  menu.flowPos = null
  menu.visible = true
}

function onNodeContextMenu(payload: NodeMouseEvent): void {
  payload.event.preventDefault()
  menuNodeId.value = payload.node.id
  editor.selectNode(payload.node.id)
  openMenu(
    payload.event as MouseEvent,
    [
      { id: 'duplicate', label: '复制节点', icon: ICON.copy },
      { separator: true },
      { id: 'delete', label: '删除节点', icon: ICON.trash, danger: true },
    ],
    payload.node.data.label || '节点',
  )
}

function onEdgeContextMenu(payload: EdgeMouseEvent): void {
  payload.event.preventDefault()
  menuEdgeId.value = payload.edge.id
  editor.selectEdge(payload.edge.id)
  openMenu(
    payload.event as MouseEvent,
    [
      { id: 'reverse', label: '反转方向', icon: ICON.reverse },
      { separator: true },
      { id: 'delete-edge', label: '删除链路', icon: ICON.trash, danger: true },
    ],
    typeof payload.edge.label === 'string' && payload.edge.label ? payload.edge.label : '攻击链路',
  )
}

function onPaneContextMenu(event: MouseEvent): void {
  event.preventDefault()
  // 节点/连线上的右键由 vue-flow 的 node/edge contextmenu 处理，这里只处理空白处
  const target = event.target as HTMLElement | null
  if (target?.closest('.vue-flow__node') || target?.closest('.vue-flow__edge')) return
  try {
    menu.flowPos = screenToFlowCoordinate({ x: event.clientX, y: event.clientY })
  } catch {
    menu.flowPos = null
  }
  openMenu(event, [
    { id: 'new-host', label: '新建主机', icon: ICON.plus },
    { id: 'new-web', label: '新建 Web 应用', icon: ICON.plus },
    { id: 'new-domain', label: '新建域控', icon: ICON.plus },
    { separator: true },
    { id: 'fit', label: '适应视图', icon: ICON.fit, shortcut: 'F' },
    { id: 'export', label: '导出 PNG', icon: ICON.image },
  ])
}

/** 菜单项分发 */
function onMenuSelect(id: string): void {
  const pos = menu.flowPos ?? { x: 220, y: 180 }
  const newKinds: Record<string, string> = { 'new-host': 'host', 'new-web': 'web', 'new-domain': 'domain' }
  if (newKinds[id]) {
    createAsset(newKinds[id], { x: pos.x - 106, y: pos.y - 42 })
    return
  }
  if (id === 'duplicate' && menuNodeId.value) editor.duplicateNode(menuNodeId.value)
  if (id === 'delete' && menuNodeId.value) editor.removeNode(menuNodeId.value)
  if (id === 'reverse' && menuEdgeId.value) editor.reverseEdge(menuEdgeId.value)
  if (id === 'delete-edge' && menuEdgeId.value) editor.removeEdge(menuEdgeId.value)
  if (id.startsWith('to-') && linkSource.value) {
    const target = id.slice(3)
    const result = editor.addEdge(linkSource.value, target)
    if (!result.ok && result.reason === 'duplicate') ui.toast('两个节点之间已存在链路', 'warn')
    linkSource.value = ''
    return
  }
  if (id === 'fit') doFit()
  if (id === 'export') void exportPng()
}

/* 导出与保存 */

/** 导出 PNG */
async function exportPng(): Promise<void> {
  if (!wrapper.value || !editor.project) return
  const selection = { node: editor.selectedNodeId, edge: editor.selectedEdgeId }
  editor.clearSelection()
  try {
    fitView({ padding: 0.12, duration: 0 })
    await nextTick()
    await new Promise((resolve) => setTimeout(resolve, 180))
    const result = await exportCanvasPng(
      wrapper.value,
      `${safeFilename(editor.project.name)}-${fileStamp()}.png`,
      2,
    )
    ui.toast(`已导出 PNG ${result.width}×${result.height}`, 'ok')
  } catch (err) {
    ui.toast(err instanceof Error ? err.message : '导出失败', 'error')
  } finally {
    if (selection.node) editor.selectNode(selection.node)
    if (selection.edge) editor.selectEdge(selection.edge)
  }
}

/** PNG 预览：只渲染不下载，先看效果再决定保存 */
async function openPngPreview(): Promise<void> {
  if (!wrapper.value || !editor.project || pngBusy.value) return
  pngBusy.value = true
  const selection = { node: editor.selectedNodeId, edge: editor.selectedEdgeId }
  editor.clearSelection()
  try {
    fitView({ padding: 0.12, duration: 0 })
    await nextTick()
    await new Promise((resolve) => setTimeout(resolve, 180))
    pngShot.value = await renderCanvasPng(wrapper.value, 2)
    showPng.value = true
  } catch (err) {
    ui.toast(err instanceof Error ? err.message : '预览失败', 'error')
  } finally {
    pngBusy.value = false
    if (selection.node) editor.selectNode(selection.node)
    if (selection.edge) editor.selectEdge(selection.edge)
  }
}

/** 从预览里保存 */
async function savePng(): Promise<void> {
  if (!pngShot.value || !editor.project) return
  try {
    await saveImage(pngShot.value.dataUrl, `${safeFilename(editor.project.name)}-${fileStamp()}.png`)
    ui.toast(`已保存 ${pngShot.value.width}×${pngShot.value.height}`, 'ok')
    showPng.value = false
  } catch (err) {
    ui.toast(err instanceof Error ? err.message : '保存失败', 'error')
  }
}

/** 导出 JSON */
async function exportJson(): Promise<void> {
  if (!editor.project) return
  try {
    const bundle = await projectApi.exportProject(editor.project.id)
    const blob = new Blob([JSON.stringify(bundle, null, 2)], { type: 'application/json;charset=utf-8' })
    downloadBlob(blob, `${safeFilename(editor.project.name)}-${fileStamp()}.atlas.json`)
    ui.toast('JSON 已导出', 'ok')
  } catch (err) {
    ui.toast(err instanceof Error ? err.message : '导出失败', 'error')
  }
}

/** 手动保存 */
async function save(): Promise<void> {
  const ok = await editor.save({ silent: false })
  ui.toast(ok ? '已保存' : editor.conflict ? '存在版本冲突' : '保存失败', ok ? 'ok' : 'warn')
}

/* 键盘 */

function onKeydown(event: KeyboardEvent): void {
  const tag = (event.target as HTMLElement | null)?.tagName
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return

  if (event.key === 'Escape') {
    if (menu.visible) {
      menu.visible = false
      return
    }
    editor.clearSelection()
    return
  }
  if (event.key === 'Delete' || event.key === 'Backspace') {
    if (editor.selectedNodeId || editor.selectedEdgeId) {
      event.preventDefault()
      if (editor.selectedNodeId) editor.removeNode(editor.selectedNodeId)
      else if (editor.selectedEdgeId) editor.removeEdge(editor.selectedEdgeId)
    }
    return
  }
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 's') {
    event.preventDefault()
    void save()
    return
  }
  if (!event.ctrlKey && !event.metaKey && event.key.toLowerCase() === 'f') {
    event.preventDefault()
    doFit()
  }
}

/* 生命周期 */

watch(
  projectId,
  async (id) => {
    if (!id) return
    await catalog.load()
    await editor.load(id)
    if (editor.error) {
      ui.toast(editor.error, 'error')
      void router.replace({ name: 'projects' })
      return
    }
    await nextTick()
    window.setTimeout(doFit, 220)
  },
  { immediate: true },
)

onMounted(() => {
  window.addEventListener('keydown', onHotkey)
  window.addEventListener('keydown', onKeydown)
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown)
  window.removeEventListener('keydown', onHotkey)
  editor.reset()
})

/** 返回项目列表 */
async function goBack(): Promise<void> {
  if (editor.dirty) {
    const ok = await ui.confirm({
      title: '离开项目',
      message: '有尚未保存的改动，离开后将丢失。',
      confirmText: '离开',
      danger: true,
    })
    if (!ok) return
  }
  void router.push({ name: 'projects' })
}
</script>

<template>
  <div class="app">
    <header class="topbar">
      <a class="brand" href="#/" @click.prevent="goBack">
        <span class="brand__mark">
          <svg aria-hidden="true" viewBox="0 0 24 24"><path d="M4 18V7l6 7 3-4 5 8" /></svg>
        </span>
        <span class="brand__text">Atlas</span>
      </a>

      <div class="topbar__crumb">
        <span class="topbar__crumb-sep">/</span>
        <span class="topbar__title">{{ editor.project?.name || '加载中' }}</span>
        <span class="chip" :class="editor.permission === 'owner' ? 'chip--person' : 'chip--system'">
          {{ PERMISSION_LABEL[editor.permission] }}
        </span>
        <span
          class="chip"
          :class="
            saveState.tone === 'ok'
              ? 'chip--ok'
              : saveState.tone === 'bad'
                ? 'chip--bad'
                : saveState.tone === 'warn'
                  ? 'chip--warn'
                  : ''
          "
        >
          {{ saveState.text }}
        </span>
        <span class="mono" style="font-size: 10px; color: var(--color-muted)">
          v{{ editor.project?.version ?? 0 }}
        </span>
      </div>

      <span class="topbar__spacer" />

      <div v-if="remotePresence.length" class="presence" :title="remotePresence.map((p) => p.userName).join('、')">
        <span v-for="member in remotePresence.slice(0, 4)" :key="member.userId" class="presence__item">
          {{ member.userName.slice(0, 1) }}
        </span>
        <span v-if="remotePresence.length > 4" class="presence__item presence__item--more">
          +{{ remotePresence.length - 4 }}
        </span>
      </div>

      <button class="btn btn--sm btn--ghost" :disabled="!editor.canManage" @click="showMembers = true">成员授权</button>
      <button class="btn btn--sm btn--ghost" @click="showPassword = true">修改口令</button>
      <button class="btn btn--sm btn--ghost" title="撤销上一步（Ctrl+Z）" @click.stop="undo()">撤销</button>
      <button class="btn btn--sm btn--ghost" :disabled="!editor.canEdit || !editor.nodes.length" @click="editor.tidy()">格式化</button>
      <button class="btn btn--sm btn--ghost" :disabled="!editor.nodes.length" @click="openPngPreview">预览PNG</button>
      <button class="btn btn--sm btn--ghost" @click="exportPng">导出PNG</button>
      <button class="btn btn--sm btn--ghost" @click="exportJson">导出配置</button>
      <button class="btn btn--sm btn--primary" :disabled="!editor.dirty || !editor.canEdit" @click="save">
        保存
      </button>
      <button class="btn btn--sm btn--ghost" @click="goBack">返回</button>
    </header>

    <div class="editor">
      <PalettePanel
        @add="(slug: string) => editor.canEdit && createAsset(slug, { x: 220, y: 180 })"
        @manage="showTypes = true"
      />

      <div
        ref="wrapper"
        class="canvas-wrap"
        @dragover="onDragOver"
        @drop="onDrop"
        @dblclick="onCanvasDoubleClick"
        @contextmenu.prevent="onPaneContextMenu"
      >
        <VueFlow
          :id="flowId"
          v-model:nodes="nodes"
          v-model:edges="edges"
          :node-types="nodeTypes"
          :edge-types="edgeTypes"
          :min-zoom="0.12"
          :max-zoom="2.2"
          :delete-key-code="null"
          :connect-on-click="false"
          :connection-mode="ConnectionMode.Loose"
          :elevate-edges-on-select="true"
          class="atlas-flow"
          @connect="onConnect"
          @connect-start="onConnectStart"
          @connect-end="onConnectEnd"
          @node-click="onNodeClick"
          @node-double-click="onNodeDoubleClick"
          @edge-click="onEdgeClick"
          @pane-click="onPaneClick"
          @node-context-menu="onNodeContextMenu"
          @edge-context-menu="onEdgeContextMenu"
        >
          <Background :gap="24" :size="1" pattern-color="var(--color-rule)" />
          <Controls position="bottom-left" :show-interactive="false" />
          <MiniMap
            position="bottom-right"
            :pannable="true"
            :zoomable="true"
            :node-border-radius="3"
            mask-color="rgba(10,14,20,0.7)"
          />
        </VueFlow>

        <div class="node-search atlas-export-hide">
          <div class="node-search__box">
            <svg aria-hidden="true" viewBox="0 0 24 24"><circle cx="11" cy="11" r="7" /><path d="m20 20-3.2-3.2" /></svg>
            <input
              ref="searchInput"
              v-model="searchKeyword"
              class="input"
              type="search"
              placeholder="搜索节点"
              aria-label="搜索节点"
              spellcheck="false"
              @keydown="onSearchKey"
              @focus="searchOpen = searchKeyword.trim().length > 0"
            />
          </div>
          <ul v-if="searchOpen && searchHits.length" class="node-search__list" role="listbox">
            <li v-for="(hit, index) in searchHits" :key="hit.node.id">
              <button
                type="button"
                class="node-search__item"
                :class="{ 'is-active': index === searchIndex }"
                @mouseenter="searchIndex = index"
                @click="focusNode(hit.node)"
              >
                <span class="node-search__name">{{ hit.node.data.label || '未命名' }}</span>
                <span class="node-search__meta mono">{{ hit.node.data.ip || hit.node.data.domain || '' }}</span>
              </button>
            </li>
          </ul>
          <p v-else-if="searchOpen" class="node-search__empty">没有匹配的节点</p>
        </div>

        <div class="canvas-hud atlas-export-hide">
          <span><b>{{ editor.stats.nodes }}</b> 节点</span>
          <span><b>{{ editor.stats.edges }}</b> 链路</span>
          <span class="chip chip--ok">实时</span>
        </div>

        <div v-if="editor.conflict" class="conflict atlas-export-hide">
          <span>他人已更新到 v{{ editor.conflict.version }}，你的改动尚未写入</span>
          <button class="btn btn--sm" @click="editor.reloadLatest()">载入服务器版本</button>
          <button class="btn btn--sm btn--primary" @click="editor.forceSave()">用我的覆盖</button>
        </div>

        <div v-if="isEmpty" class="canvas-empty">
          <div class="state" style="max-width: 420px">
            <h3>从左侧拖入第一个资产</h3>
          </div>
        </div>

        <div v-if="editor.loading" class="canvas loading-mask">
          <div class="skeleton" style="width: 220px">
            <span /><span /><span />
          </div>
        </div>

        <div v-if="editor.error" class="canvas">
          <div class="state state--error">
            <h3>无法打开项目</h3>
            <p>{{ editor.error }}</p>
            <button class="btn btn--sm" @click="goBack">返回项目列表</button>
          </div>
        </div>

        <div class="export-watermark" data-export-watermark>
          <b>{{ editor.project?.name }}</b>
          <span class="export-watermark__meta">
            <span>{{ new Date().toLocaleString('zh-CN') }}</span>
            <span>RedTeam Atlas</span>
            <span>{{ auth.displayName }}</span>
          </span>
        </div>
      </div>

      <InspectorPanel
        @manage-members="showMembers = true"
        @manage-types="showTypes = true"
      />
    </div>

    <ContextMenu
      :visible="menu.visible"
      :x="menu.x"
      :y="menu.y"
      :title="menu.title"
      :items="menu.items"
      @select="onMenuSelect"
      @close="menu.visible = false"
    />

    <AssetTypeDialog :visible="showTypes" @close="showTypes = false" />
    <MemberDialog :visible="showMembers" @close="showMembers = false" />
    <VulnPanel
      :visible="showVuln"
      :node="vulnNode"
      :project-name="editor.project?.name ?? ''"
      :kind-label="vulnNode ? resolveType(vulnNode.data.kind, catalog.types).label : ''"
      :author="auth.displayName"
      :editable="editor.canEdit"
      @close="showVuln = false"
      @save="onSaveVuln"
      @preview="onPreviewVulns"
    />
    <ChangePasswordDialog :visible="showPassword" @close="showPassword = false" />
    <PngPreviewDialog
      :visible="showPng"
      :data-url="pngShot?.dataUrl ?? ''"
      :width="pngShot?.width ?? 0"
      :height="pngShot?.height ?? 0"
      :bytes="pngShot?.bytes ?? 0"
      @close="showPng = false"
      @save="savePng"
    />
    <ReportPreview
      :visible="showPreview"
      :meta="reportMeta"
      :vulns="previewVulns"
      @close="showPreview = false"
    />
  </div>
</template>

<style scoped>
.presence {
  display: flex;
  margin-right: var(--space-xs);
}

.presence__item {
  width: 22px;
  height: 22px;
  border-radius: 4px;
  display: grid;
  place-items: center;
  font-size: 10px;
  color: var(--color-accent);
  background: var(--color-accent-soft);
  border: 1px solid var(--color-paper);
  margin-left: -4px;
}

.presence__item:first-child {
  margin-left: 0;
}

.presence__item--more {
  color: var(--color-muted);
  background: var(--color-paper-2);
}

.atlas-flow {
  width: 100%;
  height: 100%;
}
.conflict {
  position: absolute;
  top: var(--space-xs);
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  gap: var(--space-xs);
  padding: var(--space-2xs) var(--space-xs);
  border: 1px solid color-mix(in oklab, var(--color-bad) 55%, transparent);
  border-radius: var(--radius-pill);
  background: var(--color-bad-soft);
  color: var(--color-bad);
  font-size: var(--text-xs);
  z-index: var(--z-raised);
}

.canvas-empty,
.canvas.loading-mask,
.canvas {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
  pointer-events: none;
  background: color-mix(in oklab, var(--color-paper-3) 86%, transparent);
  z-index: 5;
}

.canvas-empty .state,
.canvas .state {
  pointer-events: auto;
  background: var(--color-paper);
}

.export-watermark__meta {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-sm);
}

.export-watermark {
  position: absolute;
  right: var(--space-sm);
  bottom: var(--space-sm);
  text-align: right;
  opacity: 0;
  pointer-events: none;
  z-index: 4;
}

.export-watermark b {
  display: block;
  font-size: var(--text-xs);
  font-weight: 500;
  color: var(--color-ink-2);
}

.export-watermark span {
  font-family: var(--font-outlier);
  font-size: 10px;
  color: var(--color-muted);
}
</style>
