<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { fetchLatestRelease, fetchVersion, isNewerVersion, runUpgrade } from '@/api/system'
import { useUiStore } from '@/stores/ui'
import type { GithubRelease, VersionInfo } from '@/types'

const props = defineProps<{ visible: boolean }>()
const emit = defineEmits<{ close: [] }>()

const ui = useUiStore()

const info = ref<VersionInfo | null>(null)
const release = ref<GithubRelease | null>(null)
const loading = ref(true)
const phase = ref<'idle' | 'downloading' | 'restarting' | 'done'>('idle')
const note = ref('')

/** 最新版本：优先官网 Releases，取不到时用本机已知值 */
const latest = computed(() => {
  const version = release.value?.tagName.replace(/^v/i, '') ?? info.value?.latest ?? ''
  const newer = release.value ? isNewerVersion(release.value.tagName, info.value?.current ?? '') : !!info.value?.updateAvailable
  return { version, newer }
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

/** 查一次本机版本与官网最新发布 */
async function check(): Promise<void> {
  loading.value = true
  release.value = null
  try {
    info.value = await fetchVersion()
    release.value = await fetchLatestRelease(info.value.checkUrl)
  } catch {
    // 取不到就只显示当前版本，不打断使用
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

/** 一键更新 */
async function upgrade(): Promise<void> {
  if (!canUpgrade.value) return
  phase.value = 'downloading'
  note.value = ''
  try {
    const result = await runUpgrade()
    phase.value = 'restarting'
    note.value = `已下载 ${(result.bytes / 1048576).toFixed(1)} MB，正在重启`
    await waitRestart(result.to)
  } catch (err) {
    phase.value = 'idle'
    note.value = err instanceof Error ? err.message : '更新失败'
    ui.toast(note.value, 'error')
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
        </div>

        <p v-if="note" class="ver__note">{{ note }}</p>
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

.ver__note {
  font-size: var(--text-xs);
  line-height: 1.6;
  color: var(--color-ink-2);
}

.modal__spacer {
  flex: 1;
}
</style>