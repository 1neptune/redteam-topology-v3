<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'

export interface MenuItem {
  id?: string
  label?: string
  icon?: string
  shortcut?: string
  danger?: boolean
  separator?: boolean
}

const props = defineProps<{
  visible: boolean
  x: number
  y: number
  title?: string
  items: MenuItem[]
}>()

const emit = defineEmits<{ select: [id: string]; close: [] }>()

const root = ref<HTMLElement | null>(null)

/** 点击外部关闭 */
function onDocPointer(event: PointerEvent): void {
  if (!props.visible) return
  const target = event.target as Node | null
  if (root.value && target && root.value.contains(target)) return
  emit('close')
}

function onKeydown(event: KeyboardEvent): void {
  if (props.visible && event.key === 'Escape') emit('close')
}

onMounted(() => {
  document.addEventListener('pointerdown', onDocPointer, true)
  document.addEventListener('keydown', onKeydown)
})

onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', onDocPointer, true)
  document.removeEventListener('keydown', onKeydown)
})

/** 选择菜单项 */
function choose(item: MenuItem): void {
  if (item.separator || !item.id) return
  emit('select', item.id)
  emit('close')
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="visible"
      ref="root"
      class="menu"
      role="menu"
      :style="{ left: `${x}px`, top: `${y}px` }"
    >
      <div v-if="title" class="menu__head">{{ title }}</div>
      <template v-for="(item, index) in items" :key="`${item.id ?? 'sep'}-${index}`">
        <div v-if="item.separator" class="menu__sep" />
        <button
          v-else
          class="menu__item"
          :class="{ 'menu__item--danger': item.danger }"
          role="menuitem"
          @click="choose(item)"
        >
          <svg v-if="item.icon" aria-hidden="true" viewBox="0 0 24 24"><path :d="item.icon" /></svg>
          <span class="menu__label">{{ item.label }}</span>
          <span v-if="item.shortcut" class="menu__shortcut mono">{{ item.shortcut }}</span>
        </button>
      </template>
    </div>
  </Teleport>
</template>