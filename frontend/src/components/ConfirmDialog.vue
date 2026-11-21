<script setup lang="ts">
import { useUiStore } from '@/stores/ui'

const ui = useUiStore()
</script>

<template>
  <div v-if="ui.confirmState.visible" class="modal-mask" @click.self="ui.settle(false)">
    <div class="modal modal--sm" role="dialog" aria-modal="true">
      <div class="modal__head">{{ ui.confirmState.title }}</div>
      <div class="modal__body">
        <p class="confirm-text">{{ ui.confirmState.message }}</p>
      </div>
      <div class="modal__foot">
        <button class="btn" @click="ui.settle(false)">
          {{ ui.confirmState.cancelText || '取消' }}
        </button>
        <button
          class="btn"
          :class="ui.confirmState.danger ? 'btn--danger' : 'btn--primary'"
          @click="ui.settle(true)"
        >
          {{ ui.confirmState.confirmText || '确定' }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.confirm-text {
  font-size: var(--text-sm);
  line-height: 1.7;
  color: var(--color-ink-2);
  white-space: pre-wrap;
}
</style>