<script setup lang="ts">
import { computed } from 'vue'
import { BaseEdge, EdgeLabelRenderer, getBezierPath, type Position } from '@vue-flow/core'
import type { EdgeStatus, GraphField } from '@/types'

const props = defineProps<{
  id: string
  sourceX: number
  sourceY: number
  targetX: number
  targetY: number
  sourcePosition: Position
  targetPosition: Position
  selected?: boolean
  markerEnd?: string
  data?: {
    status?: EdgeStatus
    notes?: string
    fields?: GraphField[]
    animated?: boolean
  }
  /** 两端资产名，用于属性块的抬头 */
  endpoints?: string
}>()

const geometry = computed(() =>
  getBezierPath({
    sourceX: props.sourceX,
    sourceY: props.sourceY,
    sourcePosition: props.sourcePosition,
    targetX: props.targetX,
    targetY: props.targetY,
    targetPosition: props.targetPosition,
  }),
)

const path = computed(() => geometry.value[0])
const labelX = computed(() => geometry.value[1])
const labelY = computed(() => geometry.value[2])

const status = computed<EdgeStatus>(() => props.data?.status ?? 'planned')

const STATUS_TEXT: Record<EdgeStatus, string> = {
  planned: '计划中',
  active: '进行中',
  success: '已成功',
  failed: '已失败',
}

const strokeColor = computed(() => {
  if (props.selected) return 'var(--color-accent)'
  if (status.value === 'success') return 'var(--color-ok)'
  if (status.value === 'failed') return 'var(--color-bad)'
  if (status.value === 'active') return 'var(--color-cool)'
  return 'var(--color-rule-2)'
})

const strokeWidth = computed(() => (props.selected ? 2.2 : status.value === 'success' ? 1.9 : 1.4))
const dashed = computed(() => status.value === 'planned')

/** 画布上要显示的属性行：备注与自定义字段，空值不占位 */
const rows = computed<Array<{ label: string; value: string }>>(() => {
  const list: Array<{ label: string; value: string }> = []
  const notes = (props.data?.notes ?? '').trim()
  if (notes) list.push({ label: '备注', value: notes })
  for (const field of props.data?.fields ?? []) {
    const value = field.value?.trim()
    if (value) list.push({ label: field.key, value })
  }
  return list
})

/** 属性块只在有内容时显示，避免空框堆在画布上 */
const visible = computed(() => rows.value.length > 0)
</script>

<template>
  <BaseEdge
    :id="id"
    :path="path"
    :marker-end="markerEnd"
    :interaction-width="20"
    :style="{ stroke: strokeColor, strokeWidth, strokeDasharray: dashed ? '5 4' : undefined }"
  />

  <EdgeLabelRenderer v-if="visible">
    <div
      class="edge-card"
      :class="{ 'is-selected': selected }"
      :style="{ transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)` }"
    >
      <span class="edge-card__state" :data-state="status">{{ STATUS_TEXT[status] }}</span>
      <dl class="edge-card__rows">
        <template v-for="row in rows" :key="row.label">
          <dt>{{ row.label }}</dt>
          <dd :title="row.value">{{ row.value }}</dd>
        </template>
      </dl>
    </div>
  </EdgeLabelRenderer>
</template>

<style scoped>
.edge-card {
  position: absolute;
  pointer-events: all;
  width: 168px;
  padding: var(--space-2xs) var(--space-xs);
  border: 1px solid var(--color-rule-2);
  border-radius: var(--radius-card);
  background: var(--color-paper-2);
  font-size: 10px;
  line-height: 1.5;
  color: var(--color-ink-2);
}

.edge-card.is-selected {
  border-color: var(--color-accent);
}

.edge-card__state {
  display: block;
  margin-bottom: var(--space-3xs);
  font-size: 9px;
  color: var(--color-muted);
}

.edge-card__state[data-state='success'] {
  color: var(--color-ok);
}

.edge-card__state[data-state='failed'] {
  color: var(--color-bad);
}

.edge-card__state[data-state='active'] {
  color: var(--color-cool);
}

.edge-card__rows {
  display: grid;
  grid-template-columns: 32px minmax(0, 1fr);
  gap: 1px var(--space-2xs);
  margin: 0;
}

.edge-card__rows dt {
  color: var(--color-muted);
  font-size: 9px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.edge-card__rows dd {
  margin: 0;
  min-width: 0;
  font-family: var(--font-outlier);
  overflow-wrap: anywhere;
  white-space: pre-wrap;
}
</style>