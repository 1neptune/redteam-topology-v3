<script setup lang="ts">
import { computed, ref } from 'vue'
import { formatClock, formatStamp } from '@/lib/format'
import type { AuditRecord } from '@/types'

const props = defineProps<{
  records: AuditRecord[]
  loading?: boolean
}>()

const emit = defineEmits<{ more: [] }>()

/** 动作码 → 类型标签，只在 summary 缺失时兜底 */
const ACTION_LABEL: Record<string, string> = {
  'node.create': '节点',
  'node.update': '节点',
  'node.delete': '节点',
  'edge.create': '链路',
  'edge.update': '链路',
  'edge.delete': '链路',
  'project.create': '项目',
  'project.update': '项目',
  'project.delete': '项目',
  'member.grant': '授权',
  'member.revoke': '授权',
  'asset_type.create': '组件',
  'asset_type.update': '组件',
  'asset_type.delete': '组件',
}

const keyword = ref('')
const from = ref('')
const to = ref('')

/** 用户名去重，作为筛选候选 */
const names = computed(() => {
  const set = new Set<string>()
  for (const record of props.records) {
    if (record.userName) set.add(record.userName)
  }
  return Array.from(set).sort()
})

/** 起止日期转毫秒；只有日期时补到当天首尾 */
function boundary(value: string, end: boolean): number | null {
  if (!value) return null
  const date = new Date(`${value}T${end ? '23:59:59' : '00:00:00'}`)
  const ms = date.getTime()
  return Number.isNaN(ms) ? null : ms
}

/** 把历史记录里的「新增节点「X」」统一成「新增 X 节点」 */
function normalize(summary: string): string {
  // 名称本身已带类型词（如「未命名链路」）时不再重复追加
  const withKind = (verb: string, kind: string, name: string): string =>
    name.endsWith(kind) ? `${verb} ${name}` : `${verb} ${name} ${kind}`
  return summary
    .replace(/^新增(节点|链路)「(.+?)」$/, (_m, kind: string, name: string) => withKind('新增', kind, name))
    .replace(/^删除(节点|链路)「(.+?)」$/, (_m, kind: string, name: string) => withKind('删除', kind, name))
    .replace(/^修改(节点|链路)「(.+?)」：(.+)$/, (_m, kind: string, name: string, brief: string) =>
      `${withKind('修改', kind, name)}：${brief}`,
    )
    .replace(/^修改(节点|链路)「(.+?)」$/, (_m, kind: string, name: string) => withKind('修改', kind, name))
    .replace(/^(新增|删除|修改) (链路|节点) 节点的/, '$1 $2')
}

const rows = computed(() => {
  const start = boundary(from.value, false)
  const end = boundary(to.value, true)
  const kw = keyword.value.trim().toLowerCase()
  return props.records
    .filter((record) => {
      if (start !== null && record.createdAt < start) return false
      if (end !== null && record.createdAt > end) return false
      if (kw && !record.userName.toLowerCase().includes(kw)) return false
      return true
    })
    .map((record) => ({
      ...record,
      // 摘要已带具体对象名与字段值，缺失时退回动作标签
      text: record.summary
        ? normalize(record.summary)
        : `${ACTION_LABEL[record.action] ?? record.action} ${record.targetLabel ?? ''}`.trim(),
      // 说明、详情、建议属于正文，不给在审计里逐字比对；历史记录一并过滤
      changes: (record.changes ?? [])
        .filter((c) => !/漏洞说明|渗透详情|处置建议/.test(c.field))
        .slice(0, 4),
    }))
})

/** 清空筛选 */
function reset(): void {
  keyword.value = ''
  from.value = ''
  to.value = ''
}
</script>

<template>
  <div class="audit">
    <div class="audit__filters">
      <input
        v-model="keyword"
        class="input input--sm audit__user"
        type="search"
        size="10"
        list="audit-names"
        placeholder="用户名"
        aria-label="按用户名搜索"
      />

      <input v-model="from" class="input input--sm" type="date" aria-label="起始日期" />
      <input v-model="to" class="input input--sm" type="date" aria-label="结束日期" />
      <button v-if="keyword || from || to" class="btn btn--sm btn--ghost" @click="reset">清空</button>
    </div>

    <datalist id="audit-names">
      <option v-for="name in names" :key="name" :value="name" />
    </datalist>

    <p v-if="!rows.length && !loading" class="audit__empty">
      {{ keyword || from || to ? '没有匹配的记录' : '还没有变更记录' }}
    </p>

    <div class="trail">
      <div v-for="record in rows" :key="record.id" class="trail__row">
        <span class="trail__time" :title="formatStamp(record.createdAt)">{{ formatClock(record.createdAt) }}</span>
        <div class="trail__body">
          <div class="trail__what">
            <b>{{ record.userName }}</b>{{ record.text }}
          </div>
          <div v-if="record.changes.length" class="trail__diff">
            <div v-for="(change, index) in record.changes" :key="index" class="trail__diff-row">
              <b>{{ change.field }}</b>
              <i>{{ change.from || '空' }}</i>
              <em>{{ change.to || '空' }}</em>
            </div>
          </div>
        </div>
      </div>
    </div>

    <button class="btn btn--sm btn--block" :disabled="loading" @click="emit('more')">
      {{ loading ? '读取中' : '加载更早' }}
    </button>
  </div>
</template>

<style scoped>
.audit {
  display: flex;
  flex-direction: column;
  gap: var(--space-xs);
  min-height: 0;
  height: 100%;
  overflow: hidden;
}

.audit__filters {
  display: grid;
  grid-template-columns: 10ch minmax(0, 1fr) minmax(0, 1fr) auto;
  gap: var(--space-2xs);
  align-items: center;
}

.audit__filters .input--sm {
  height: var(--control-h-sm);
  font-size: var(--text-xs);
}

/* 用户名搜索框按 10 个字符定宽 */
.audit__user {
  width: 10ch;
  min-width: 10ch;
  padding-inline: var(--space-2xs);
}

.audit__empty {
  font-size: var(--text-xs);
  color: var(--color-muted);
}

/* 记录列表吃掉剩余高度并在内部滚动，不会把面板撑长 */
.trail {
  flex: 1;
  min-height: 0;
  display: grid;
  align-content: start;
  gap: var(--space-2xs);
  overflow-y: auto;
  overscroll-behavior: contain;
}

.trail__what {
  font-size: var(--text-xs);
  line-height: 1.6;
  color: var(--color-ink-2);
  overflow-wrap: anywhere;
}

.trail__what b {
  margin-right: var(--space-2xs);
  font-weight: 500;
  color: var(--color-accent);
}
</style>