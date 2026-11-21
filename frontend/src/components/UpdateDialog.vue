<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { fetchVersion, isNewerVersion, runUpgrade } from '@/api/system'
import { useUiStore } from '@/stores/ui'
import type { VersionInfo } from '@/types'

const props = defineProps<{ visible: boolean }>()
const emit = defineEmits<{ close: [] }>()

const ui = useUiStore()

const info = ref<VersionInfo | null>(null)
const loading = ref(true)
const phase = ref<'idle' | 'downloading' | 'restarting' | 'done'>('idle')
const error = ref('')
const note = ref('')

/** 最新版本：全部来自后端（manifest 或 GitHub，后端已做缓存与降级） */
const latest = computed(() => {
  const version = info.value?.latest ?? ''
  const newer = !!info.value?.updateAvailable && isNewerVersion(version, info.value?.current ?? '')
  return { version, newer }
})

/** 版本来源的可读说明 */
const sourceText = computed(() => {
  switch (info.value?.source) {
    case 'manifest':
      return '来自本地版本清单'
    case 'github':
      return '来自 GitHub Releases'
    default:
      return '未取到最新版本信息'
  }
})

const canUpgrade = computed(() => latest.value.newer && phase.value === 'idle')

watch(
  () => props.visible,
  (open) => {
    if (open) {
      phase.value = 'idle'
      note.value = ''
      void check()
    }
  },
  { immediate: true },
)

/**
 * 查一次版本信息。
 *
 * 只问后端：浏览器直连 api.github.com 在很多网络下是不通的，
 * 由服务端代查（并缓存），拿不到时后端会如实返回 source: none。
 */
async function check(): Promise<void> {
  loading.value = true

  error.value = ''
  try {
    info.value = await fetchVersion()
  } catch (err) {
    error.value = err instanceof Error ? err.message : '读取版本信息失败'
  } finally {
    loading.value = false
  }
}

function openHome(): void {
  const url = info.value?.homepage
  if (url) window.open(url, '_blank', 'noopener')
}

const sleep = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms))

/** 等新进程起来并确认版本已换 */
async function waitRestart(target: string): Promise<void> {
  for (let i = 0; i < 40; i += 1) {
    await sleep(1500)
    try {
      const next = await fetchVersion()
      if (!isNewerVersion(target, next.current)) {
        phase.value = 'done'
        note.value = `已更新到 ${next.current}，即将重新载入`
        await sleep(900)
        window.location.reload()
        return
      }
    } catch {
      // 进程正在替换重启，继续等
    }
  }
  phase.value = 'idle'
  note.value = '重启超时，请手动确认服务状态'
}

/** 容器部署时后端返回的升级命令 */
const manualCommand = ref('')

/** 一键更新 */
async function upgrade(): Promise<void> {
  if (!canUpgrade.value) return
  phase.value = 'downloading'
  note.value = ''
  manualCommand.value = ''
  try {
    const result = await runUpgrade()
    if (result.mode === 'manual') {
      // 容器不能重建自身：把要在宿主机跑的命令显示出来，用户复制即可
      phase.value = 'idle'
      manualCommand.value = result.command || './scripts/upgrade.sh'
      note.value = `容器部署：在宿主机执行下面的命令升级到 ${result.to}`
      return
    }
    phase.value = 'restarting'
    note.value = `已下载 ${(result.bytes / 1048576).toFixed(1)} MB，正在重启`
    await waitRestart(result.to)
  } catch (err) {
    phase.value = 'idle'
    note.value = err instanceof Error ? err.message : '更新失败'
    ui.toast(note.value, 'error')
  }
}

/** 复制命令到剪贴板 */
async function copyCommand(): Promise<void> {
  try {
    await navigator.clipboard.writeText(manualCommand.value)
    ui.toast('命令已复制', 'ok')
  } catch {
    ui.toast('复制失败，请手动选中', 'error')
  }
}
</script>

<template>
  <div v-if="visible" class="modal-mask" @click.self="emit('close')">
    <div class="modal modal--slim" role="dialog" aria-modal="true" aria-label="版本">
      <div class="modal__head">版本</div>

      <div class="modal__body">
        <div class="ver">
          <div class="ver__row">
            <span>当前版本</span>
            <b class="mono">{{ info?.current ?? '—' }}</b>
          </div>
          <div class="ver__row">
            <span>最新版本</span>
            <b class="mono" :class="{ 'is-new': latest.newer }">
              {{ loading ? '检查中' : latest.version || '—' }}
            </b>
          </div>
          <div class="ver__row">
            <span>版本来源</span>
            <b class="ver__src">{{ loading ? '—' : sourceText }}</b>
          </div>
        </div>

        <p v-if="error" class="ver__note is-err">{{ error }}</p>
        <p v-else-if="!loading && !latest.newer && info?.source === 'none'" class="ver__note">
          服务端连不上 GitHub，也没有本地版本清单。在宿主机执行升级脚本会自动写入清单。
        </p>

        <p v-if="note" class="ver__note">{{ note }}</p>

        <!-- 容器部署：给出要在宿主机跑的命令，一键复制 -->
        <div v-if="manualCommand" class="ver__cmd">
          <code class="ver__cmd-text mono">cd /opt/atlas && {{ manualCommand }}</code>
          <button class="btn btn--sm btn--ghost" @click="copyCommand">复制</button>
        </div>
      </div>

      <div class="modal__foot">
        <button
          class="btn btn--primary"
          :disabled="!canUpgrade"
          :title="latest.newer ? '' : '当前已是最新版本'"
          @click="upgrade"
        >
          {{ phase === 'idle' ? '一键更新' : '更新中' }}
        </button>
        <button class="btn" :disabled="!info" @click="openHome">官网</button>
        <span class="modal__spacer" />
        <button class="btn" @click="emit('close')">关闭</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.modal--slim {
  max-width: 360px;
}

.ver {
  display: grid;
  gap: var(--space-2xs);
}

.ver__row {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-sm);
  padding: var(--space-xs) 0;
  border-bottom: 1px solid var(--color-rule);
  font-size: var(--text-xs);
  color: var(--color-muted);
}

.ver__row:last-child {
  border-bottom: 0;
}

.ver__row b {
  font-size: var(--text-md);
  font-weight: 400;
  color: var(--color-ink);
}

.ver__row b.is-new {
  color: var(--color-accent);
}

.ver__src {
  font-size: var(--text-xs);
  font-weight: 400;
  color: var(--color-muted);
}

.ver__note.is-err {
  color: var(--color-danger);
}

.ver__cmd {
  display: flex;
  align-items: center;
  gap: var(--space-xs);
  margin-top: var(--space-xs);
  padding: var(--space-xs) var(--space-sm);
  background: var(--color-sunken);
  border: var(--rule-hair) solid var(--color-rule);
  border-radius: var(--radius-sm);
}

.ver__cmd-text {
  flex: 1;
  min-width: 0;
  font-size: var(--text-xs);
  color: var(--color-ink-2);
  overflow-wrap: anywhere;
  user-select: all;
}

.ver__note {
  font-size: var(--text-xs);
  line-height: 1.6;
  color: var(--color-ink-2);
}

.modal__spacer {
  flex: 1;
}
</style>