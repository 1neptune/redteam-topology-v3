<script setup lang="ts">
import { computed } from 'vue'
import { builtinOf } from '@/lib/iconSet'
import { useCatalogStore } from '@/stores/catalog'

/**
 * 组件图标：优先渲染 5 个内置图标（SVG），其次是用户上传的图片。
 * 两者都没有时渲染插槽（调用方放内置字形兜底）。
 */
const props = withDefaults(
  defineProps<{
    iconUrl?: string | null
    size?: number
  }>(),
  { iconUrl: null, size: 14 },
)

const catalog = useCatalogStore()

const builtin = computed(() => builtinOf(props.iconUrl))
const uploaded = computed(() => (builtin.value ? '' : catalog.iconSrc(props.iconUrl)))
const box = computed(() => ({ width: `${props.size}px`, height: `${props.size}px` }))
</script>

<template>
  <svg v-if="builtin" class="type-icon" :style="box" aria-hidden="true" viewBox="0 0 24 24">
    <path :d="builtin.path" />
  </svg>
  <img v-else-if="uploaded" class="type-icon" :style="box" :src="uploaded" alt="" />
  <slot v-else />
</template>

<style scoped>
.type-icon {
  display: block;
  object-fit: contain;
  border-radius: 2px;
}

svg.type-icon {
  fill: none;
  stroke: currentColor;
  stroke-width: 1.6;
  stroke-linecap: round;
  stroke-linejoin: round;
}
</style>