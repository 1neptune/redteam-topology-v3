<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import type { GraphNode, RiskLevel, Vulnerability } from '@/types'
import { RISK_COLOR_VAR, RISK_LABEL, RISK_SOFT_VAR, riskKey, vulnTitle } from '@/lib/report'
import { newVulnId } from '@/lib/ids'
import RichEditor from '@/components/RichEditor.vue'
import { useUiStore } from '@/stores/ui'

const props = defineProps<{
  visible: boolean
  node: GraphNode | null
  projectName: string
  kindLabel: string
  author: string
  /** 只读权限时为 false */
  editable: boolean
}>()

const emit = defineEmits<{
  close: []
  save: [vulns: Vulnerability[]]
  preview: [vulns: Vulnerability[]]
}>()

const ui = useUiStore()

/** 五个风险等级，按严重程度排列 */
const RISKS: RiskLevel[] = ['critical', 'high', 'medium', 'low', 'info']

function blank(): Vulnerability {
  return {
    id: newVulnId(),
    name: '',
    description: '',
    risk: '',
    detail: '',
    remediation: '',
    updatedAt: null,
    updatedBy: '',
  }
}

function clone(input: Vulnerability): Vulnerability {
  return {
    id: input.id || newVulnId(),
    name: input.name ?? '',
    description: input.description ?? '',
    risk: (input.risk ?? '') as RiskLevel | '',
    detail: input.detail ?? '',
    remediation: input.remediation ?? '',
    updatedAt: input.updatedAt ?? null,
    updatedBy: input.updatedBy ?? '',
  }
}

const draft = ref<Vulnerability[]>([])
const baseline = ref('[]')
const active = ref(0)
const saving = ref(false)

const dirty = computed(() => JSON.stringify(draft.value) !== baseline.value)
const current = computed(() => draft.value[active.value] ?? null)
const currentRisk = computed(() => riskKey(current.value?.risk))

/** 打开时从节点复制一份，避免直接改到画布数据 */
watch(
  () => props.visible,
  (open) => {
    if (!open) return
    const source = (props.node?.data.vulns ?? []).map(clone)
    draft.value = source
    baseline.value = JSON.stringify(source)
    active.value = source.length ? 0 : -1
  },
  { immediate: true },
)

/** 追加一条空漏洞并选中 */
function addVuln(): void {
  if (!props.editable) return
  draft.value = [...draft.value, blank()]
  active.value = draft.value.length - 1
}

/** 删除指定条目 */
function removeVuln(index: number): void {
  if (!props.editable) return
  const next = draft.value.filter((_, i) => i !== index)
  draft.value = next
  if (!next.length) {
    active.value = -1
  } else {
    active.value = Math.min(active.value, next.length - 1)
  }
}

/** 选风险等级，再点一次取消 */
function pickRisk(risk: RiskLevel): void {
  const item = current.value
  if (!props.editable || !item) return
  item.risk = item.risk === risk ? '' : risk
}

/** 保存整组漏洞 */
function save(): void {
  if (!props.editable || saving.value) return
  saving.value = true
  const payload = draft.value.map(clone)
  emit('save', payload)
  baseline.value = JSON.stringify(payload)
  saving.value = false
}

/** 预览报告（未保存也按当前草稿渲染） */
function preview(): void {
  const payload = draft.value.map(clone).filter((item) => !isBlank(item))
  if (!payload.length) {
    ui.toast('还没有漏洞记录', 'warn')
    return
  }
  emit('preview', payload)
}

function isBlank(item: Vulnerability): boolean {
  return (
    !item.name.trim() &&
    !item.description.trim() &&
    !item.risk &&
    !item.detail.trim() &&
    !item.remediation.trim()
  )
}

/** Esc 关闭 */
function onKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape' && props.visible) {
    event.stopPropagation()
    emit('close')
  }
}

onMounted(() => window.addEventListener('keydown', onKeydown, true))
onUnmounted(() => window.removeEventListener('keydown', onKeydown, true))
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="vuln" @click.self="emit('close')">
      <aside class="vuln__panel" role="dialog" aria-modal="true" aria-label="渗透测试漏洞">
        <header class="vuln__head">
          <div class="vuln__head-main">
            <span class="vuln__node mono">{{ node?.data.label || '节点' }}</span>
            <h2 class="vuln__title">渗透测试漏洞</h2>
          </div>
          <button class="btn btn--sm btn--ghost" @click="emit('close')">关闭</button>
        </header>

        <div class="vuln__list">
          <button
            v-for="(item, index) in draft"
            :key="item.id"
            type="button"
            class="vrow"
            :class="{ 'is-on': index === active }"
            @click="active = index"
          >
            <span class="vrow__no mono">{{ index + 1 }}</span>
            <span class="vrow__name" :title="vulnTitle(item, index)">
              {{ item.name.trim() || '未命名漏洞' }}
            </span>
            <span
              v-if="item.risk"
              class="vrow__risk"
              :style="{ color: RISK_COLOR_VAR[riskKey(item.risk)], background: RISK_SOFT_VAR[riskKey(item.risk)] }"
            >
              {{ RISK_LABEL[riskKey(item.risk)] }}
            </span>
            <span
              v-if="editable"
              class="vrow__del"
              role="button"
              tabindex="0"
              aria-label="删除该漏洞"
              @click.stop="removeVuln(index)"
              @keydown.enter.stop="removeVuln(index)"
            >
              ×
            </span>
          </button>

          <button v-if="editable" type="button" class="vadd" @click="addVuln">添加漏洞</button>
          <p v-if="!draft.length" class="vuln__empty">该资产没有漏洞记录</p>
        </div>

        <div v-if="current" class="vuln__body">
          <div class="field">
            <label class="field__label" for="vuln-name">漏洞名称</label>
            <input
              id="vuln-name"
              v-model="current.name"
              class="input"
              :readonly="!editable"
              placeholder="任意文件上传漏洞"
              maxlength="120"
            />
          </div>

          <div class="field">
            <span class="field__label">风险等级</span>
            <div class="risk-row" role="group" aria-label="风险等级">
              <button
                v-for="risk in RISKS"
                :key="risk"
                type="button"
                class="risk"
                :class="{ 'is-on': currentRisk === risk }"
                :style="{ '--risk-color': RISK_COLOR_VAR[risk], '--risk-soft': RISK_SOFT_VAR[risk] }"
                :aria-pressed="currentRisk === risk"
                :disabled="!editable"
                @click="pickRisk(risk)"
              >
                {{ RISK_LABEL[risk] }}
              </button>
            </div>
          </div>

          <div class="field">
            <label class="field__label" for="vuln-desc">漏洞说明</label>
            <textarea
              id="vuln-desc"
              v-model="current.description"
              class="input textarea"
              rows="3"
              :readonly="!editable"
              placeholder="影响范围与成因"
            />
          </div>

          <div class="field">
            <label class="field__label">渗透测试详情</label>
            <RichEditor :key="current.id" v-model="current.detail" :disabled="!editable" />
          </div>

          <div class="field">
            <label class="field__label" for="vuln-fix">处置建议</label>
            <textarea
              id="vuln-fix"
              v-model="current.remediation"
              class="input textarea"
              rows="3"
              :readonly="!editable"
              placeholder="修复方案与复测方式"
            />
          </div>
        </div>

        <footer class="vuln__foot">
          <button class="btn btn--sm" :disabled="!draft.length" @click="preview">预览报告</button>
          <div class="vuln__actions">
            <button class="btn btn--sm btn--ghost" @click="emit('close')">取消</button>
            <button
              class="btn btn--sm btn--primary"
              :disabled="!editable || !dirty || saving"
              @click="save"
            >
              {{ dirty ? '保存' : '已保存' }}
            </button>
          </div>
        </footer>
      </aside>
    </div>
  </Teleport>
</template>

<style scoped>
.vuln {
  position: fixed;
  inset: 0;
  z-index: var(--z-modal);
  display: flex;
  justify-content: flex-end;
  background: var(--color-scrim);
}

.vuln__panel {
  width: 600px;
  max-width: 100%;
  height: 100dvh;
  display: flex;
  flex-direction: column;
  background: var(--color-paper);
  border-left: 1px solid var(--color-rule);
  box-shadow: var(--shadow-panel);
}

.vuln__head {
  flex: none;
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-sm);
  padding: var(--space-md) var(--space-lg);
  border-bottom: 1px solid var(--color-rule);
}

.vuln__node {
  font-size: 11px;
  color: var(--color-cool);
}

.vuln__title {
  font-size: var(--text-lg);
  letter-spacing: -0.02em;
  margin-top: var(--space-3xs);
}

.vuln__list {
  flex: none;
  max-height: 34%;
  overflow-y: auto;
  padding: var(--space-xs) var(--space-sm);
  display: grid;
  gap: var(--space-3xs);
  align-content: start;
  border-bottom: 1px solid var(--color-rule);
  background: var(--color-paper-2);
}

.vrow {
  display: flex;
  align-items: center;
  gap: var(--space-xs);
  width: 100%;
  padding: var(--space-2xs) var(--space-xs);
  border: 1px solid transparent;
  border-radius: var(--radius-pill);
  background: transparent;
  color: var(--color-ink-2);
  font: inherit;
  font-size: var(--text-xs);
  text-align: left;
  cursor: pointer;
}

.vrow:hover {
  background: var(--color-paper-3);
}

.vrow.is-on {
  background: var(--color-paper);
  border-color: var(--color-rule-2);
  color: var(--color-ink);
}

.vrow:focus-visible {
  outline: 2px solid var(--color-focus);
  outline-offset: 2px;
}

.vrow__no {
  flex: none;
  width: 16px;
  font-size: 10px;
  color: var(--color-muted);
}

.vrow__name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.vrow__risk {
  flex: none;
  padding: 0 var(--space-2xs);
  border-radius: var(--radius-pill);
  font-size: 9.5px;
  line-height: 1.6;
}

.vrow__del {
  flex: none;
  width: 18px;
  height: 18px;
  display: grid;
  place-items: center;
  border-radius: var(--radius-pill);
  color: var(--color-muted);
  font-size: 14px;
  line-height: 1;
  cursor: pointer;
}

.vrow__del:hover {
  color: var(--color-bad);
  background: var(--color-bad-soft);
}

.vadd {
  justify-self: start;
  padding: var(--space-2xs) var(--space-xs);
  border: 1px dashed var(--color-rule-2);
  border-radius: var(--radius-pill);
  background: transparent;
  color: var(--color-muted);
  font: inherit;
  font-size: var(--text-xs);
  cursor: pointer;
}

.vadd:hover {
  color: var(--color-ink);
  border-color: var(--color-accent);
}

.vadd:focus-visible {
  outline: 2px solid var(--color-focus);
  outline-offset: 2px;
}

.vuln__empty {
  padding: var(--space-xs) var(--space-2xs);
  font-size: var(--text-xs);
  color: var(--color-muted);
}

.vuln__body {
  flex: 1;
  overflow-y: auto;
  padding: var(--space-md) var(--space-lg);
  display: grid;
  gap: var(--space-md);
  align-content: start;
}

.textarea {
  resize: vertical;
  min-height: 68px;
  line-height: 1.65;
  font-family: inherit;
  padding: var(--space-xs) var(--space-sm);
}

.risk-row {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2xs);
}

.risk {
  padding: var(--space-2xs) var(--space-sm);
  border: 1px solid var(--color-rule);
  border-radius: var(--radius-pill);
  background: transparent;
  color: var(--color-muted);
  font: inherit;
  font-size: var(--text-xs);
  cursor: pointer;
}

.risk:hover:not(:disabled) {
  color: var(--color-ink);
  border-color: var(--color-rule-2);
}

.risk.is-on {
  color: var(--risk-color);
  background: var(--risk-soft);
  border-color: var(--risk-color);
}

.risk:disabled {
  cursor: not-allowed;
  opacity: 0.6;
}

.risk:focus-visible {
  outline: 2px solid var(--color-focus);
  outline-offset: 2px;
}

.vuln__foot {
  flex: none;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-sm);
  padding: var(--space-sm) var(--space-lg);
  border-top: 1px solid var(--color-rule);
  background: var(--color-paper-2);
}

.vuln__actions {
  display: flex;
  gap: var(--space-2xs);
}

@media (max-width: 40rem) {
  .vuln__panel {
    width: 100%;
  }
  .vuln__foot {
    flex-direction: column;
    align-items: stretch;
  }
}
</style>