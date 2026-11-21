<script setup lang="ts">
import { computed, ref } from 'vue'
import { useCatalogStore } from '@/stores/catalog'
import { useEditorStore } from '@/stores/editor'
import { useUiStore } from '@/stores/ui'
import { EDGE_STATUSES, NODE_STATUSES, resolveType } from '@/lib/nodeTypes'
import { ellipsis } from '@/lib/format'
import FieldRows from './FieldRows.vue'
import AuditTrail from './AuditTrail.vue'
import type { EdgeData, GraphField, GraphNode, NodeData, NodeStatus, EdgeStatus } from '@/types'

const emit = defineEmits<{ 'manage-types': []; 'manage-members': [] }>()

const editor = useEditorStore()
const catalog = useCatalogStore()
const saving = ref(false)
const ui = useUiStore()

const tab = ref<'props' | 'audit'>('props')

const node = computed(() => editor.selectedNode)
const edge = computed(() => editor.selectedEdge)
const readonly = computed(() => !editor.canEdit)
const nodeType = computed(() => (node.value ? resolveType(node.value.data.kind, catalog.types) : null))

const edgeEnds = computed(() => {
  if (!edge.value) return { from: '', to: '' }
  const from = editor.nodes.find((n: GraphNode) => n.id === edge.value?.source)
  const to = editor.nodes.find((n: GraphNode) => n.id === edge.value?.target)
  return { from: from?.data.label || '未知节点', to: to?.data.label || '未知节点' }
})

const breakdown = computed(() => {
  const total = editor.stats.nodes || 1
  return NODE_STATUSES.map((status) => ({
    label: status.label,
    tone: status.tone,
    count: editor.stats.byStatus[status.value] ?? 0,
    pct: ((editor.stats.byStatus[status.value] ?? 0) / total) * 100,
  })).filter((item) => item.count > 0)
})

const TONE_COLOR: Record<string, string> = {
  '': 'var(--color-rule-2)',
  system: 'var(--color-cool)',
  warn: 'var(--color-accent)',
  ok: 'var(--color-ok)',
  bad: 'var(--color-bad)',
}

const PERMISSION_LABEL: Record<string, string> = { owner: '管理', editor: '可编辑', viewer: '只读' }

/** 更新节点字段 */
function patchNode(patch: Partial<NodeData>): void {
  if (node.value) editor.updateNode(node.value.id, patch)
}

/** 更新连线字段 */
function patchEdge(patch: { label?: string; animated?: boolean; data?: Partial<EdgeData> }): void {
  if (editor.selectedEdgeId) editor.updateEdge(editor.selectedEdgeId, patch)
}

/** 把面板里的改动立即写库（平时走防抖自动保存，这里是显式触发） */
async function commit(): Promise<void> {
  if (!editor.canEdit) return
  saving.value = true
  try {
    const ok = await editor.save()
    ui.toast(ok ? '已保存' : '没有需要保存的改动', ok ? 'ok' : undefined)
  } catch (err) {
    ui.toast(err instanceof Error ? err.message : '保存失败', 'error')
  } finally {
    saving.value = false
  }
}

/** 删除当前选中元素 */
async function removeSelected(): Promise<void> {
  if (node.value) {
    const ok = await ui.confirm({
      title: '删除节点',
      message: `将删除「${node.value.data.label || '未命名'}」及其关联链路。`,
      confirmText: '删除',
      danger: true,
    })
    if (ok) editor.removeNode(node.value.id)
    return
  }
  if (edge.value) {
    const ok = await ui.confirm({
      title: '删除链路',
      message: '该链路会从拓扑中移除。',
      confirmText: '删除',
      danger: true,
    })
    if (ok) editor.removeEdge(edge.value.id)
  }
}
</script>

<template>
  <aside class="inspector" aria-label="检视器">
    <div class="inspector__tabs" role="tablist">
      <button class="inspector__tab" role="tab" :aria-selected="tab === 'props'" @click="tab = 'props'">
        属性
      </button>
      <button class="inspector__tab" role="tab" :aria-selected="tab === 'audit'" @click="tab = 'audit'">
        审计记录 <span class="mono">{{ editor.audit.length }}</span>
      </button>
    </div>

    <div v-if="tab === 'props'" class="inspector__body">
      <div v-if="readonly" class="chip chip--system">只读权限，无法修改</div>

      <!-- 节点 -->
      <template v-if="node">
        <div class="inspector__head">
          <div class="inspector__heading">
            <div class="inspector__title">{{ node.data.label || '未命名节点' }}</div>
            <div class="inspector__sub">{{ nodeType?.label }}<span class="inspector__sep">节点</span></div>
          </div>
          <button class="icon-btn" title="取消选择" @click="editor.clearSelection()">
            <svg aria-hidden="true" viewBox="0 0 24 24"><path d="M6 6l12 12M18 6L6 18" /></svg>
          </button>
        </div>

        <div class="field">
          <label class="field__label" for="node-label">名称</label>
          <input
            id="node-label"
            class="input"
            :value="node.data.label"
            :disabled="readonly"
            placeholder="10.0.0.5 或 OA 门户"
            @input="patchNode({ label: ($event.target as HTMLInputElement).value })"
          />
        </div>

        <div class="field">
          <label class="field__label" for="node-kind">组件类型</label>
          <select
            id="node-kind"
            class="select"
            :value="node.data.kind"
            :disabled="readonly"
            @change="patchNode({ kind: ($event.target as HTMLSelectElement).value })"
          >
            <option v-for="type in catalog.types" :key="type.slug" :value="type.slug">
              {{ type.label }}{{ type.builtin ? '' : '（自定义）' }}
            </option>
          </select>
        </div>

        <div class="field">
          <span class="field__label">控制状态</span>
          <div class="segmented segmented--4" role="group">
            <button
              v-for="status in NODE_STATUSES"
              :key="status.value"
              type="button"
              :aria-pressed="node.data.status === status.value"
              :disabled="readonly"
              @click="patchNode({ status: status.value as NodeStatus })"
            >
              {{ status.label }}
            </button>
          </div>
        </div>

        <div class="field-row">
          <div class="field">
            <label class="field__label" for="node-ip">IP / 主机</label>
            <input
              id="node-ip"
              class="input mono"
              :value="node.data.ip"
              :disabled="readonly"
              placeholder="10.0.0.5"
              @input="patchNode({ ip: ($event.target as HTMLInputElement).value })"
            />
          </div>
          <div class="field">
            <label class="field__label" for="node-port">端口</label>
            <input
              id="node-port"
              class="input mono"
              :value="node.data.port"
              :disabled="readonly"
              placeholder="445"
              @input="patchNode({ port: ($event.target as HTMLInputElement).value })"
            />
          </div>
        </div>

        <div class="field">
          <label class="field__label" for="node-domain">域名</label>
          <input
            id="node-domain"
            class="input mono"
            :value="node.data.domain"
            :disabled="readonly"
            placeholder="oa.example.com"
            @input="patchNode({ domain: ($event.target as HTMLInputElement).value })"
          />
        </div>

        <div class="field">
          <label class="field__label" for="node-os">操作系统 / 环境</label>
          <input
            id="node-os"
            class="input"
            :value="node.data.os"
            :disabled="readonly"
            placeholder="Windows Server 2019"
            @input="patchNode({ os: ($event.target as HTMLInputElement).value })"
          />
        </div>


        <div class="field">
          <span class="field__label">自定义字段</span>
          <FieldRows
            :fields="node.data.fields"
            :disabled="readonly"
            @change="patchNode({ fields: $event as GraphField[] })"
          />
        </div>

        <div class="field">
          <label class="field__label" for="node-notes">备注</label>
          <textarea
            id="node-notes"
            class="textarea"
            :value="node.data.notes"
            :disabled="readonly"
            placeholder="已获取的权限、下一步动作"
            @input="patchNode({ notes: ($event.target as HTMLTextAreaElement).value })"
          />
        </div>

        <div v-if="!readonly" class="inspector__actions">
          <button class="btn btn--sm btn--primary" :disabled="saving" @click="commit">
            {{ saving ? '保存中' : '保存' }}
          </button>
          <button class="btn btn--sm" @click="editor.duplicateNode(node.id)">复制</button>
          <button class="btn btn--sm btn--danger" @click="removeSelected">删除</button>
        </div>
      </template>

      <!-- 连线 -->
      <template v-else-if="edge">
        <div class="inspector__head">
          <div class="inspector__heading">
            <div class="inspector__title">攻击链路</div>
            <div class="inspector__sub">
              {{ ellipsis(edgeEnds.from, 12) }} → {{ ellipsis(edgeEnds.to, 12) }}
            </div>
          </div>
          <button class="icon-btn" title="取消选择" @click="editor.clearSelection()">
            <svg aria-hidden="true" viewBox="0 0 24 24"><path d="M6 6l12 12M18 6L6 18" /></svg>
          </button>
        </div>


        <div class="field">
          <span class="field__label">链路状态</span>
          <div class="segmented segmented--4" role="group">
            <button
              v-for="status in EDGE_STATUSES"
              :key="status.value"
              type="button"
              :aria-pressed="edge.data.status === status.value"
              :disabled="readonly"
              @click="patchEdge({ data: { status: status.value as EdgeStatus } })"
            >
              {{ status.label }}
            </button>
          </div>
        </div>

        <label class="inline-check">
          <input
            type="checkbox"
            :checked="edge.data.animated"
            :disabled="readonly"
            @change="patchEdge({ animated: ($event.target as HTMLInputElement).checked })"
          />
          <span>流动动画</span>
        </label>

        <div class="field">
          <span class="field__label">自定义字段</span>
          <FieldRows
            :fields="edge.data.fields"
            :disabled="readonly"
            @change="patchEdge({ data: { fields: $event as GraphField[] } })"
          />
        </div>

        <div class="field">
          <label class="field__label" for="edge-notes">备注</label>
          <textarea
            id="edge-notes"
            class="textarea"
            :value="edge.data.notes"
            :disabled="readonly"
            placeholder="利用条件、所需凭据"
            @input="patchEdge({ data: { notes: ($event.target as HTMLTextAreaElement).value } })"
          />
        </div>

        <div v-if="!readonly" class="inspector__actions">
          <button class="btn btn--sm btn--primary" :disabled="saving" @click="commit">
            {{ saving ? '保存中' : '保存' }}
          </button>
          <button class="btn btn--sm" @click="editor.reverseEdge(edge.id)">反转方向</button>
          <button class="btn btn--sm btn--danger" @click="removeSelected">删除链路</button>
        </div>
      </template>

      <!-- 项目概览 -->
      <template v-else>
        <div class="inspector__head">
          <div class="inspector__heading">
            <div class="inspector__title">项目概览</div>
            <div class="inspector__sub">{{ editor.project?.name || '未命名项目' }}</div>
          </div>
          <button v-if="editor.canManage" class="icon-btn" title="成员授权" @click="emit('manage-members')">
            <svg aria-hidden="true" viewBox="0 0 24 24">
              <path d="M16 20v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2M9 10a4 4 0 1 0 0-8 4 4 0 0 0 0 8M19 8v6M22 11h-6" />
            </svg>
          </button>
        </div>

        <div class="readings">
          <div class="reading">
            <b>{{ editor.stats.nodes }}</b>
            <span>资产节点</span>
          </div>
          <div class="reading">
            <b>{{ editor.stats.edges }}</b>
            <span>攻击链路</span>
          </div>
        </div>

        <div v-if="breakdown.length" class="meter">
          <span class="field__label">控制状态</span>
          <div class="meter__bar">
            <span
              v-for="item in breakdown"
              :key="item.label"
              class="meter__seg"
              :style="{ width: `${item.pct}%`, background: TONE_COLOR[item.tone] }"
              :title="`${item.label} ${item.count}`"
            />
          </div>
          <div class="meter__legend">
            <span v-for="item in breakdown" :key="item.label">{{ item.label }} {{ item.count }}</span>
          </div>
        </div>

        <div class="field">
          <span class="field__label">成员与权限</span>
          <ul class="members">
            <li v-for="member in editor.members" :key="member.userId">
              <span class="members__name">{{ member.displayName }}</span>
              <span class="chip" :class="member.permission === 'owner' ? 'chip--person' : member.permission === 'editor' ? 'chip--system' : ''">
                {{ PERMISSION_LABEL[member.permission] }}
              </span>
            </li>
            <li v-if="!editor.members.length" class="members__empty">尚未授权其他成员</li>
          </ul>
        </div>

        <div class="field">
          <span class="field__label">最近变更</span>
          <button class="btn btn--sm btn--block" @click="tab = 'audit'">
            查看审计记录（{{ editor.audit.length }}）
          </button>
        </div>
      </template>
    </div>

    <div v-else class="inspector__body inspector__body--audit">
      <div class="inspector__head">
        <div class="inspector__heading">
          <div class="inspector__title">审计记录</div>
        </div>
        <span class="chip chip--system mono">{{ editor.audit.length }}</span>
      </div>
      <div class="inspector__audit">
        <AuditTrail :records="editor.audit" :loading="editor.auditLoading" @more="editor.loadAudit(false)" />
      </div>
    </div>
  </aside>
</template>

<style scoped>
/* 审计区固定高度：记录只在内部滚动，不会把面板越撑越长 */
/* 审计面板：整块撑满，只有中间的记录列表滚 */
.inspector__body--audit {
  display: flex;
  flex-direction: column;
  gap: var(--space-xs);
  overflow: hidden;
  align-content: stretch;
  padding-bottom: var(--space-xs);
}

.inspector__audit {
  flex: 1;
  min-height: 0;
  display: flex;
  overflow: hidden;
}

/* scoped 的 > * 匹配不到子组件根元素，必须用 :deep 才让审计组件铺满并交出滚动 */
.inspector__audit :deep(.audit) {
  flex: 1;
  min-height: 0;
  height: 100%;
  overflow: hidden;
}

.inspector__actions {
  display: flex;
  gap: var(--space-xs);
  flex-wrap: wrap;
  padding-top: var(--space-xs);
  border-top: 1px solid var(--color-rule);
}

.inspector__heading {
  min-width: 0;
  flex: 1;
}

.inline-check {
  display: flex;
  align-items: center;
  gap: var(--space-xs);
  font-size: var(--text-xs);
  color: var(--color-ink-2);
  cursor: pointer;
}

.inline-check input {
  accent-color: var(--color-accent);
}

.readings {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--space-xs);
}

.reading {
  display: grid;
  gap: 1px;
  padding: var(--space-xs);
  border: 1px solid var(--color-rule);
  border-radius: var(--radius-card);
}

.reading b {
  font-family: var(--font-outlier);
  font-size: var(--text-xl);
  font-weight: 400;
  color: var(--color-ink);
  line-height: 1.1;
}

.reading span {
  font-size: 10.5px;
  color: var(--color-muted);
}

.members {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  gap: var(--space-3xs);
}

.members li {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-xs);
  font-size: var(--text-xs);
}

.members__name {
  color: var(--color-accent);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.members__empty {
  color: var(--color-muted);
  font-size: 11px;
}

</style>