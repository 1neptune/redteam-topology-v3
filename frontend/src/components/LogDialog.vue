<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { fetchLogs, type LogPayload } from '@/api/system'
import { useUiStore } from '@/stores/ui'

const props = defineProps<{ visible: boolean }>()
const emit = defineEmits<{ close: [] }>()

const ui = useUiStore()
const kind = ref<'access' | 'error'>('access')
const payload = ref<LogPayload | null>(null)
const loading = ref(false)
const expanded = ref('')

/** 当前 tab 对应的表头 */
const heads = computed(() => {
  if (kind.value === 'error') return ['时间', '级别', '来源 IP', '接口', '状态码', '错误信息']
  return ['时间', '来源 IP', '接口', '请求值', '状态码', '返回值']
})

/** 读日志 */
async function load(): Promise<void> {
  if (loading.value) return
  loading.value = true
  try {
    payload.value = await fetchLogs(kind.value)
  } catch (err) {
    ui.toast(err instanceof Error ? err.message : '读取日志失败', 'error')
    payload.value = null
  } finally {
    loading.value = false
  }
}

watch(
  () => props.visible,
  (open) => {
    if (!open) return
    kind.value = 'access'
    expanded.value = ''
    void load()
  },
)

watch(kind, () => {
  expanded.value = ''
  void load()
})

/** 状态码配色 */
function statusTone(code: string): string {
  const value = Number(code)
  if (value >= 500) return 'chip--bad'
  if (value >= 400) return 'chip--warn'
  return 'chip--ok'
}
</script>

<template>
  <div v-if="visible" class="modal-mask" @click.self="emit('close')">
    <div class="modal log" role="dialog" aria-modal="true" aria-label="运行日志">
      <div class="modal__head">
        运行日志
        <div class="log__tabs" role="tablist">
          <button
            type="button"
            role="tab"
            :aria-selected="kind === 'access'"
            @click="kind = 'access'"
          >
            接口访问
          </button>
          <button
            type="button"
            role="tab"
            :aria-selected="kind === 'error'"
            @click="kind = 'error'"
          >
            错误
          </button>
        </div>
        <button class="btn btn--sm btn--ghost" :disabled="loading" @click="load">
          {{ loading ? '读取中' : '刷新' }}
        </button>
      </div>

      <div class="modal__body log__body">
        <div class="log__scroll">
          <table class="log__table">
            <thead>
              <tr>
                <th v-for="head in heads" :key="head">{{ head }}</th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="(row, index) in payload?.rows ?? []"
                :key="index"
                class="log__row"
                @click="expanded = expanded === `${kind}-${index}` ? '' : `${kind}-${index}`"
              >
                <td class="mono log__cell log__cell--time">{{ row[0] }}</td>
                <template v-if="kind === 'error'">
                  <td class="log__cell">{{ row[1] }}</td>
                  <td class="mono log__cell">{{ row[2] }}</td>
                  <td class="mono log__cell log__cell--wide">{{ row[3] }}</td>
                  <td class="log__cell">
                    <span class="chip" :class="statusTone(row[4] ?? '')">{{ row[4] }}</span>
                  </td>
                  <td class="mono log__cell log__cell--wide">{{ row[5] }}</td>
                </template>
                <template v-else>
                  <td class="mono log__cell">{{ row[1] }}</td>
                  <td class="mono log__cell log__cell--wide">{{ row[2] }}</td>
                  <td class="mono log__cell log__cell--wide">{{ row[3] || '—' }}</td>
                  <td class="log__cell">
                    <span class="chip" :class="statusTone(row[4] ?? '')">{{ row[4] }}</span>
                  </td>
                  <td class="mono log__cell log__cell--wide">{{ row[5] || '—' }}</td>
                </template>
              </tr>
            </tbody>
          </table>

          <p v-if="!loading && !(payload?.rows ?? []).length" class="log__empty">
            还没有记录
          </p>
        </div>
      </div>

      <div class="modal__foot">
        <span class="log__count">{{ (payload?.rows ?? []).length }} 条</span>
        <button class="btn" @click="emit('close')">关闭</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.log {
  width: min(1180px, 96vw);
  max-width: 1180px;
}

.modal__head {
  display: flex;
  align-items: center;
  gap: var(--space-sm);
}

.log__tabs {
  display: flex;
  gap: var(--rule-hair);
  margin-left: auto;
  background: var(--color-rule);
  border: var(--rule-hair) solid var(--color-rule);
  border-radius: var(--radius-card);
  overflow: hidden;
}

.log__tabs button {
  padding: var(--space-3xs) var(--space-sm);
  border: 0;
  background: var(--color-paper-2);
  color: var(--color-muted);
  font: inherit;
  font-size: var(--text-xs);
  cursor: pointer;
}

.log__tabs button[aria-selected='true'] {
  background: var(--color-cool-soft);
  color: var(--color-cool);
}

.log__tabs button:focus-visible {
  outline: 2px solid var(--color-focus);
  outline-offset: -2px;
}

.log__body {
  padding: 0;
}

.log__scroll {
  max-height: 62dvh;
  overflow: auto;
}

.log__table {
  width: 100%;
  border-collapse: collapse;
  font-size: var(--text-xs);
}

.log__table th {
  position: sticky;
  top: 0;
  z-index: 1;
  padding: var(--space-2xs) var(--space-xs);
  background: var(--color-paper-3);
  border-bottom: var(--rule-hair) solid var(--color-rule-2);
  text-align: left;
  font-weight: 500;
  font-size: 11px;
  color: var(--color-muted);
  white-space: nowrap;
}

.log__row {
  cursor: pointer;
}

.log__row:nth-child(even) {
  background: color-mix(in oklab, var(--color-paper-3) 40%, transparent);
}

.log__cell {
  padding: var(--space-2xs) var(--space-xs);
  border-bottom: var(--rule-hair) solid var(--color-rule);
  vertical-align: top;
  color: var(--color-ink-2);
  max-width: 320px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.log__cell--time {
  white-space: nowrap;
  color: var(--color-muted);
}

.log__cell--wide {
  max-width: 380px;
}

.log__empty {
  padding: var(--space-lg);
  text-align: center;
  font-size: var(--text-xs);
  color: var(--color-muted);
}

.modal__foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.log__count {
  font-size: var(--text-xs);
  color: var(--color-muted);
}
</style>