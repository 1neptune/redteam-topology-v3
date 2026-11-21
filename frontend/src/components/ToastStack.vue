<script setup lang="ts">
import { useUiStore } from '@/stores/ui'

const ui = useUiStore()
</script>

<template>
  <div class="toast-stack" role="status" aria-live="polite">
    <div
      v-for="item in ui.toasts"
      :key="item.id"
      class="toast"
      :class="`toast--${item.tone}`"
      @click="ui.dismiss(item.id)"
    >
      {{ item.message }}
    </div>
  </div>
</template>

<style scoped>
.toast-stack {
  position: fixed;
  right: var(--space-md);
  bottom: var(--space-md);
  display: grid;
  gap: var(--space-2xs);
  z-index: var(--z-toast);
}

.toast {
  padding: var(--space-2xs) var(--space-sm);
  border: 1px solid var(--color-rule-2);
  border-radius: var(--radius-card);
  background: var(--color-paper-2);
  color: var(--color-ink);
  font-size: var(--text-xs);
  cursor: pointer;
  max-width: 46ch;
}

.toast--ok {
  border-color: color-mix(in oklab, var(--color-ok) 55%, transparent);
  color: var(--color-ok);
}

.toast--warn {
  border-color: color-mix(in oklab, var(--color-accent) 55%, transparent);
  color: var(--color-accent);
}

.toast--error {
  border-color: color-mix(in oklab, var(--color-bad) 55%, transparent);
  color: var(--color-bad);
}
</style>