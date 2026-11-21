<script setup lang="ts">
import { ref, watch } from 'vue'
import { changePassword } from '@/api/auth'
import { useAuthStore } from '@/stores/auth'
import { useUiStore } from '@/stores/ui'

const props = defineProps<{ visible: boolean }>()
const emit = defineEmits<{ close: [] }>()

const auth = useAuthStore()
const ui = useUiStore()

const current = ref('')
const next = ref('')
const confirm = ref('')
const busy = ref(false)
const error = ref('')

/** 口令规则，与后端一致 */
function valid(value: string): boolean {
  return value.length >= 8 && /[A-Z]/.test(value) && /[a-z]/.test(value)
}

watch(
  () => props.visible,
  (open) => {
    if (!open) return
    current.value = ''
    next.value = ''
    confirm.value = ''
    error.value = ''
  },
)

/** 提交修改 */
async function submit(): Promise<void> {
  if (busy.value) return
  error.value = ''
  if (!valid(next.value)) {
    error.value = '新口令至少 8 位，且同时包含大写和小写字母'
    return
  }
  if (next.value !== confirm.value) {
    error.value = '两次输入的新口令不一致'
    return
  }
  busy.value = true
  try {
    await changePassword({ current: current.value, next: next.value })
    ui.toast('口令已更新，请用新口令重新登录', 'ok')
    emit('close')
    await auth.logout()
  } catch (err) {
    error.value = err instanceof Error ? err.message : '修改失败'
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div v-if="visible" class="modal-mask" @click.self="emit('close')">
    <div class="modal modal--password" role="dialog" aria-modal="true" aria-label="修改口令">
      <div class="modal__head">修改口令</div>

      <div class="modal__body">
        <div class="field">
          <label class="field__label" for="pw-current">当前口令</label>
          <input id="pw-current" v-model="current" class="input" type="password" autocomplete="current-password" />
        </div>
        <div class="field">
          <label class="field__label" for="pw-next">新口令</label>
          <input id="pw-next" v-model="next" class="input" type="password" autocomplete="new-password" />
        </div>
        <div class="field">
          <label class="field__label" for="pw-confirm">再次输入</label>
          <input id="pw-confirm" v-model="confirm" class="input" type="password" autocomplete="new-password" />
        </div>
        <p v-if="error" class="pw-error">{{ error }}</p>
      </div>

      <div class="modal__foot">
        <button class="btn" @click="emit('close')">取消</button>
        <button class="btn btn--primary" :disabled="busy || !current || !next" @click="submit">
          {{ busy ? '提交中' : '确认修改' }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.modal--password {
  max-width: 380px;
}

.pw-error {
  font-size: var(--text-xs);
  line-height: 1.6;
  color: var(--color-bad);
}
</style>