<script setup lang="ts">
import { computed } from 'vue'
import { Handle, Position } from '@vue-flow/core'
import { useCatalogStore } from '@/stores/catalog'
import { useEditorStore } from '@/stores/editor'
import { resolveType, iconOf, statusMeta } from '@/lib/nodeTypes'
import TypeIcon from '@/components/TypeIcon.vue'
import { formatClock } from '@/lib/format'
import { RISK_COLOR_VAR, RISK_LABEL, RISK_SOFT_VAR, topRisk } from '@/lib/report'
import type { GraphNode } from '@/types'

const props = defineProps<{
  id: string
  data: GraphNode['data']
  selected?: boolean
}>()

const catalog = useCatalogStore()
const editor = useEditorStore()

const type = computed(() => resolveType(props.data.kind, catalog.types))
const glyph = computed(() => iconOf(type.value.slug))
const status = computed(() => statusMeta(props.data.status))
const endpoint = computed(() => {
  const ip = (props.data.ip ?? '').trim()
  const port = (props.data.port ?? '').trim()
  if (ip && port) return `${ip}:${port}`
  return ip || port
})
const visibleFields = computed(() =>
  (props.data.fields ?? [])
    .filter((field) => field.key || field.value)
    .map((field) => ({ key: field.key || '字段', value: field.value || '空' })),
)
/** 卡片信息行：IP、操作系统、标签、备注、自定义字段，标签与值成对显示 */
interface InfoRow {
  key: string
  label: string
  value?: string
  title?: string
  mono?: boolean
  chips?: string[]
}

const rows = computed<InfoRow[]>(() => {
  const list: InfoRow[] = []
  if (endpoint.value) {
    list.push({ key: 'ip', label: 'IP', value: endpoint.value, title: endpoint.value, mono: true })
  }
  if (props.data.domain?.trim()) {
    // 域名后同样带端口；域名已写端口就不再补一次
    const domain = props.data.domain.trim()
    const port = props.data.port?.trim()
    const shown = /:\d+$/.test(domain) || !port ? domain : `${domain}:${port}`
    list.push({ key: 'domain', label: '域名', value: shown, title: shown, mono: true })
  }
  if (props.data.os?.trim()) {
    list.push({ key: 'os', label: '系统', value: props.data.os.trim(), title: props.data.os.trim(), mono: true })
  }
  if (props.data.notes?.trim()) {
    list.push({ key: 'notes', label: '备注', value: props.data.notes.trim(), title: props.data.notes.trim() })
  }
  for (const [index, field] of visibleFields.value.entries()) {
    list.push({
      key: `f${index}`,
      label: field.key,
      value: field.value,
      title: `${field.key}: ${field.value}`,
      mono: true,
    })
  }
  return list
})
const lastEditor = computed(() => editor.lastEditorOf(props.id))
const risk = computed(() => {
  const list = props.data.vulns ?? []
  return list.length ? topRisk(list) : ''
})
</script>

<template>
  <div class="asset" :class="{ 'is-selected': selected }" :style="{ '--type': type.color }">
    <Handle id="tl" type="target" :position="Position.Left" class="asset__handle" />
    <Handle id="sr" type="source" :position="Position.Right" class="asset__handle" />

    <header class="asset__head">
      <span class="asset__icon">
        <TypeIcon :icon-url="type.iconUrl" :size="14">
          <svg aria-hidden="true" viewBox="0 0 24 24"><path :d="glyph" /></svg>
        </TypeIcon>
      </span>
      <span class="asset__name" :title="data.label">{{ data.label || '未命名' }}</span>
      <span
        v-if="risk"
        class="asset__risk"
        :style="{ color: RISK_COLOR_VAR[risk], background: RISK_SOFT_VAR[risk] }"
        :title="`${data.vulns?.length ?? 0} 条漏洞，最高 ${RISK_LABEL[risk]}`"
      >
        {{ RISK_LABEL[risk] }}
      </span>
    </header>

    <div class="asset__line">
      <span class="asset__kind">{{ type.label }}</span>
      <span class="chip asset__status" :class="status.tone ? `chip--${status.tone}` : ''">{{ status.label }}</span>
    </div>

    <dl v-if="rows.length" class="asset__meta">
      <template v-for="row in rows" :key="row.key">
        <dt>{{ row.label }}</dt>
        <dd :title="row.title">
          <template v-if="row.chips">
            <span v-for="tag in row.chips" :key="tag" class="chip asset__tag">{{ tag }}</span>
          </template>
          <template v-else :class="{ mono: row.mono }">{{ row.value }}</template>
        </dd>
      </template>
    </dl>

    <footer v-if="lastEditor" class="asset__foot">
      <span class="asset__who">{{ lastEditor.name }}</span>
      <span class="asset__when mono">{{ formatClock(lastEditor.at) }}</span>
    </footer>
  </div>
</template>

<style scoped>
.asset {
  width: 248px;
  border: 1px solid var(--color-rule);
  border-radius: var(--radius-card);
  background: var(--color-paper-2);
  overflow: hidden;
  transition: border-color var(--dur-micro) var(--ease-out), background-color var(--dur-micro) var(--ease-out);
}

.asset:hover {
  border-color: var(--color-rule-2);
}

.asset.is-selected {
  border-color: var(--color-accent);
  background: color-mix(in oklab, var(--color-paper-2) 88%, var(--color-accent));
}

.asset__risk {
  margin-left: auto;
  flex: none;
  padding: 0 var(--space-2xs);
  border-radius: var(--radius-pill);
  font-size: 9.5px;
  line-height: 1.6;
  white-space: nowrap;
}

.asset__head {
  display: flex;
  align-items: center;
  gap: var(--space-xs);
  padding: var(--space-xs) var(--space-xs) var(--space-2xs);
  min-width: 0;
}

.asset__icon {
  width: 20px;
  height: 20px;
  border-radius: 4px;
  display: grid;
  place-items: center;
  background: color-mix(in oklab, var(--type) 18%, var(--color-paper-3));
  border: 1px solid color-mix(in oklab, var(--type) 40%, transparent);
  flex: 0 0 auto;
}

.asset__icon .type-icon {
  width: 13px;
  height: 13px;
  object-fit: contain;
}

.asset__name {
  flex: 1;
  min-width: 0;
  font-size: 12.5px;
  font-weight: 500;
  color: var(--color-ink);
  /* 最多两行，超出才省略；完整内容在 title 里 */
  display: -webkit-box;
  -webkit-line-clamp: 2;
  line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  overflow-wrap: anywhere;
}

.asset__line {
  display: flex;
  align-items: center;
  gap: var(--space-xs);
  padding: 0 var(--space-xs) var(--space-2xs);
  min-width: 0;
}

.asset__kind {
  font-size: 10.5px;
  color: var(--color-muted);
  flex: 0 0 auto;
}

.asset__status {
  margin-left: auto;
  flex: 0 0 auto;
  height: 18px;
  padding: 0 var(--space-2xs);
  font-size: 9.5px;
}

.asset__endpoint {
  font-size: 11px;
  color: var(--color-cool);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.asset__meta {
  display: grid;
  /* 字段名列随内容变宽、最多占 40%，字段名不再被压成省略号 */
  grid-template-columns: fit-content(40%) minmax(0, 1fr);
  gap: var(--space-3xs) var(--space-2xs);
  align-items: start;
  padding: var(--space-2xs) var(--space-xs);
  background: var(--color-paper-3);
  border-top: 1px solid color-mix(in oklab, var(--color-rule) 60%, transparent);
  /* 信息行全部列出；条目过多时在同一区域内滚动，不撑破画布 */
  max-height: 420px;
  overflow-y: auto;
  overscroll-behavior: contain;
}

.asset__meta dt {
  font-size: 10px;
  color: var(--color-muted);
  /* 长字段名换行显示完整，不截断 */
  min-width: 0;
  overflow-wrap: anywhere;
  white-space: normal;
}

.asset__meta dd {
  margin: 0;
  min-width: 0;
  font-size: 10px;
  color: var(--color-ink-2);
  overflow-wrap: anywhere;
  white-space: pre-wrap;
}

.asset__meta dd.mono,
.asset__meta dd span.mono {
  font-family: var(--font-outlier);
}

.asset__tag {
  display: inline-block;
  height: 16px;
  padding: 0 var(--space-2xs);
  margin: 0 var(--space-3xs) var(--space-3xs) 0;
  font-size: 9px;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.asset__foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-xs);
  padding: var(--space-2xs) var(--space-xs);
  border-top: 1px solid color-mix(in oklab, var(--color-rule) 60%, transparent);
  font-size: 10px;
}

.asset__who {
  color: var(--color-accent);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.asset__when {
  color: var(--color-cool);
  flex: 0 0 auto;
}

/* 连接点：左右两侧，常态可见、悬停放大，拖拽即可连线 */
:deep(.asset__handle) {
  width: 11px;
  height: 11px;
  border-radius: 50%;
  background: var(--color-paper);
  border: 2px solid color-mix(in oklab, var(--type) 70%, var(--color-ink));
  opacity: 0.85;
  transition: opacity var(--dur-micro) var(--ease-out), transform var(--dur-micro) var(--ease-out),
    border-color var(--dur-micro) var(--ease-out);
}

.asset:hover :deep(.asset__handle) {
  opacity: 1;
}

:deep(.asset__handle:hover) {
  transform: scale(1.35);
  border-color: var(--color-accent);
  background: var(--color-accent-soft);
}

:deep(.asset__handle.connectingfrom),
:deep(.asset__handle.connectingto) {
  transform: scale(1.35);
  border-color: var(--color-accent);
  background: var(--color-accent);
  opacity: 1;
}
</style>