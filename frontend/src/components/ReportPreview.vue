<script setup lang="ts">
import { computed, onUnmounted, ref } from 'vue'
import type { Vulnerability } from '@/types'
import {
  RISK_COLOR_VAR,
  RISK_LABEL,
  exportVulnsPdf,
  exportVulnsWord,
  reportFileName,
  riskKey,
  stampText,
  topRisk,
  vulnTitle,
  type ReportMeta,
} from '@/lib/report'
import { useUiStore } from '@/stores/ui'
import { sanitizeHtml } from '@/lib/richText'

const props = defineProps<{
  visible: boolean
  meta: ReportMeta
  vulns: Vulnerability[]
}>()

const emit = defineEmits<{ close: [] }>()

const ui = useUiStore()
const sheet = ref<HTMLElement | null>(null)
const busy = ref<'word' | 'pdf' | ''>('')

const top = computed(() => topRisk(props.vulns))

const address = computed(() => {
  const ip = props.meta.nodeIp.trim()
  const port = props.meta.nodePort.trim()
  if (!ip) return '—'
  return port ? `${ip}:${port}` : ip
})

/** 地址加端口；已带端口就不再拼一次，没填地址返回空串 */
function withPort(host?: string, port?: string): string {
  const h = (host ?? '').trim()
  const p = (port ?? '').trim()
  if (!h) return ''
  // 例如 www.example.com:443 或 10.0.0.8:3306 已经带端口，直接返回
  if (/:\d+$/.test(h)) return h
  return p ? `${h}:${p}` : h
}

/** 报告抬头要显示的地址行：IP 与域名各一行，空的直接跳过 */
const hostLines = computed(() => {
  const lines: Array<{ label: string; value: string }> = []
  const ip = withPort(props.meta.nodeIp, props.meta.nodePort)
  const domain = withPort(props.meta.nodeDomain, props.meta.nodePort)
  if (ip) lines.push({ label: 'IP / 主机端口信息', value: ip })
  if (domain) lines.push({ label: '域名端口信息', value: domain })
  return lines
})

/** 分段文本按行渲染，保留换行 */
function lines(text: string): string[] {
  const value = text.trim()
  return value ? value.split(/\r?\n/) : ['（未填写）']
}

/** 导出 Word */
async function toWord(): Promise<void> {
  if (busy.value) return
  busy.value = 'word'
  try {
    await exportVulnsWord(props.meta, props.vulns)
    ui.toast('Word 报告已生成', 'ok')
  } catch (err) {
    ui.toast(err instanceof Error ? err.message : '导出失败', 'error')
  } finally {
    busy.value = ''
  }
}

/** 导出 PDF */
async function toPdf(): Promise<void> {
  const element = sheet.value
  if (busy.value || !element) return
  busy.value = 'pdf'
  try {
    await exportVulnsPdf(element, reportFileName(props.meta, 'pdf'))
    ui.toast('PDF 报告已生成', 'ok')
  } catch (err) {
    ui.toast(err instanceof Error ? err.message : '导出失败', 'error')
  } finally {
    busy.value = ''
  }
}

function close(): void {
  emit('close')
}

onUnmounted(() => {
  busy.value = ''
})
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="preview">
      <header class="preview__bar">
        <span class="preview__name">{{ props.meta.nodeLabel || '资产' }} 漏洞报告</span>
        <div class="preview__actions">
          <button class="btn btn--sm" :disabled="busy !== ''" @click="toWord">
            {{ busy === 'word' ? '生成中…' : '导出 Word' }}
          </button>
          <button class="btn btn--sm" :disabled="busy !== ''" @click="toPdf">
            {{ busy === 'pdf' ? '生成中…' : '导出 PDF' }}
          </button>
          <button class="btn btn--sm btn--ghost" @click="close">关闭</button>
        </div>
      </header>

      <div class="preview__stage" @click.self="close">
        <article ref="sheet" class="sheet">
          <h1 class="sheet__title">{{ props.meta.nodeLabel || '资产' }}</h1>
          <p class="sheet__sub">{{ props.meta.projectName }}　{{ props.meta.nodeKindLabel || '未分类' }}</p>

          <table class="sheet__table">
            <tbody>
              <tr v-for="line in hostLines" :key="line.label">
                <th>{{ line.label }}</th>
                <td class="mono">{{ line.value }}</td>
              </tr>
              <tr v-if="!hostLines.length">
                <th>地址</th>
                <td>{{ address }}</td>
              </tr>
              <tr>
                <th>漏洞数量</th>
                <td>{{ vulns.length }} 条</td>
              </tr>
              <tr>
                <th>最高风险</th>
                <td>
                  <span :style="{ color: RISK_COLOR_VAR[top] }">{{ RISK_LABEL[top] }}</span>
                </td>
              </tr>
              <tr>
                <th>报告生成</th>
                <td>{{ stampText(meta.generatedAt) }}　{{ props.meta.author || '未知' }}</td>
              </tr>
            </tbody>
          </table>

          <table v-if="vulns.length" class="sheet__table sheet__table--summary">
            <thead>
              <tr>
                <th class="sheet__col-no">序号</th>
                <th>漏洞名称</th>
                <th class="sheet__col-risk">风险等级</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(item, index) in vulns" :key="item.id || index">
                <td class="sheet__col-no">{{ index + 1 }}</td>
                <td>{{ vulnTitle(item, index) }}</td>
                <td class="sheet__col-risk">
                  <span :style="{ color: RISK_COLOR_VAR[riskKey(item.risk)] }">
                    {{ RISK_LABEL[riskKey(item.risk)] }}
                  </span>
                </td>
              </tr>
            </tbody>
          </table>

          <section v-for="(item, index) in vulns" :key="`s${item.id || index}`" class="sheet__section">
            <h2>
              {{ index + 1 }}. {{ vulnTitle(item, index) }}
              <span class="sheet__risk" :style="{ color: RISK_COLOR_VAR[riskKey(item.risk)] }">
                {{ RISK_LABEL[riskKey(item.risk)] }}
              </span>
            </h2>
            <h3>漏洞说明</h3>
            <p v-for="(line, i) in lines(item.description)" :key="`d${i}`">{{ line }}</p>
            <h3>渗透测试详情</h3>
            <div class="sheet__rich" v-html="sanitizeHtml(item.detail)"></div>
            <h3>处置建议</h3>
            <p v-for="(line, i) in lines(item.remediation)" :key="`r${i}`">{{ line }}</p>
          </section>

          <p class="sheet__foot">本报告由 RedTeam Atlas 生成，内容以项目内记录为准。</p>
        </article>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.preview {
  position: fixed;
  inset: 0;
  z-index: var(--z-modal);
  display: flex;
  flex-direction: column;
  background: var(--color-scrim);
  backdrop-filter: blur(2px);
}

.preview__bar {
  flex: none;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-sm);
  padding: var(--space-sm) var(--space-lg);
  background: var(--color-paper);
  border-bottom: 1px solid var(--color-rule);
}

.preview__name {
  font-size: var(--text-xs);
  color: var(--color-muted);
}

.preview__actions {
  display: flex;
  gap: var(--space-2xs);
}

.preview__stage {
  flex: 1;
  overflow-y: auto;
  padding: var(--space-lg);
  display: flex;
  justify-content: center;
  align-items: flex-start;
}

/* 报告纸面：白底 A4 宽 794px（96dpi） */
.sheet {
  width: 794px;
  max-width: 100%;
  padding: 56px 64px 64px;
  background: var(--sheet-paper);
  color: var(--sheet-ink);
  font-family: var(--font-doc);
  font-size: 13px;
  line-height: 1.75;
  border-radius: 2px;
  box-shadow: var(--shadow-sheet);
}


.sheet__title {
  margin: var(--space-3xs) 0 var(--space-2xs);
  font-size: 26px;
  line-height: 1.25;
  font-weight: 600;
  color: var(--sheet-ink);
  overflow-wrap: break-word;
  min-width: 0;
}

.sheet__sub {
  margin: 0 0 var(--space-md);
  font-size: 12px;
  color: var(--sheet-muted);
}

.sheet__table {
  width: 100%;
  border-collapse: collapse;
  margin-bottom: var(--space-sm);
}

.sheet__table th {
  width: 22%;
  padding: var(--space-2xs) var(--space-xs);
  text-align: left;
  font-weight: 400;
  font-size: 11.5px;
  color: var(--sheet-muted);
  background: var(--sheet-band);
  border: 1px solid var(--sheet-rule);
}

.sheet__table td {
  padding: var(--space-2xs) var(--space-xs);
  font-size: 12px;
  color: var(--sheet-ink-2);
  border: 1px solid var(--sheet-rule);
  overflow-wrap: break-word;
}

.sheet__table--summary thead th {
  width: auto;
  color: var(--sheet-ink-2);
  background: var(--sheet-band);
}

.sheet__col-no {
  width: 48px;
  text-align: center;
}

.sheet__col-risk {
  width: 88px;
}

.sheet__section {
  margin-top: var(--space-lg);
  padding-top: var(--space-sm);
  border-top: 1px solid var(--sheet-rule);
}

.sheet__section h2 {
  display: flex;
  align-items: baseline;
  gap: var(--space-xs);
  margin: 0 0 var(--space-xs);
  font-size: 15px;
  font-weight: 600;
  color: var(--sheet-ink);
  overflow-wrap: break-word;
  min-width: 0;
}

.sheet__risk {
  font-size: 12px;
  font-weight: 600;
}

.sheet__section h3 {
  margin: var(--space-xs) 0 var(--space-3xs);
  font-size: 12px;
  font-weight: 600;
  color: var(--sheet-muted);
}

.sheet__section p {
  margin: 0 0 var(--space-3xs);
  font-size: 12.5px;
  color: var(--sheet-ink-2);
  overflow-wrap: break-word;
  white-space: pre-wrap;
}

.sheet__rich {
  font-size: 12.5px;
  color: var(--sheet-ink-2);
  line-height: 1.75;
  overflow-wrap: break-word;
}

.sheet__rich :deep(img) {
  max-width: 100%;
  height: auto;
  display: block;
  margin: var(--space-xs) 0;
  border: 1px solid var(--sheet-rule);
}

.sheet__rich :deep(p) {
  margin: 0 0 var(--space-3xs);
}

.sheet__foot {
  margin-top: var(--space-lg);
  padding-top: var(--space-xs);
  border-top: 1px solid var(--sheet-rule);
  font-size: 10.5px;
  color: var(--sheet-muted);
}

@media (max-width: 48rem) {
  .preview__stage {
    padding: var(--space-xs);
  }
  .sheet {
    padding: var(--space-lg) var(--space-md);
  }
}
</style>
