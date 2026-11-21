<script setup lang="ts">
import { computed, ref, watch } from 'vue'

const props = defineProps<{
  visible: boolean
  dataUrl: string
  width: number
  height: number
  bytes: number
}>()

const emit = defineEmits<{ close: []; save: [] }>()

/** 缩放：默认按窗口宽度等比缩到看得全 */
const zoom = ref(1)

const sizeText = computed(() => {
  const kb = Math.round(props.bytes / 1024)
  const size = kb >= 1024 ? `${(kb / 1024).toFixed(1)} MB` : `${kb} KB`
  return `${props.width} × ${props.height}  ${size}`
})

/** 缩放到 100% 时允许拖动查看 */
const scroll = ref<HTMLDivElement | null>(null)

watch(
  () => props.visible,
  (open) => {
    if (!open) return
    zoom.value = 1
    void Promise.resolve(scroll.value?.scrollTo({ top: 0, left: 0 }))
  },
)

function onKey(event: KeyboardEvent): void {
  if (!props.visible) return
  if (event.key === 'Escape') emit('close')
}
</script>

<template>
  <div v-if="visible" class="modal-mask" @click.self="emit('close')" @keydown="onKey">
    <div class="modal modal--png" role="dialog" aria-modal="true" aria-label="PNG 预览">
      <div class="modal__head">
        <span>PNG 预览</span>
        <span class="png__size mono">{{ sizeText }}</span>
      </div>

      <div ref="scroll" class="png__stage">
        <img
          class="png__img"
          :class="{ 'is-fit': zoom <= 1 }"
          :src="dataUrl"
          :style="zoom > 1 ? { width: `${zoom * 100}%` } : undefined"
          alt="画布导出预览"
        />
      </div>

      <div class="modal__foot">
        <div class="png__zoom">
          <button class="btn btn--sm btn--ghost" aria-label="缩小" @click="zoom = Math.max(0.25, zoom - 0.25)">−</button>
          <span class="png__zoom-value mono">{{ Math.round(zoom * 100) }}%</span>
          <button class="btn btn--sm btn--ghost" aria-label="放大" @click="zoom = Math.min(3, zoom + 0.25)">+</button>
        </div>
        <div class="png__actions">
          <button class="btn btn--sm" @click="emit('close')">关闭</button>
          <button class="btn btn--sm btn--primary" @click="emit('save')">保存图片</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.modal--png {
  width: min(1120px, 100%);
}

.png__size {
  font-size: var(--text-xs);
  color: var(--color-muted);
}

/* 预览区固定高度，图放大时在框内滚动，不会把弹窗顶出屏幕 */
.png__stage {
  height: min(62dvh, 620px);
  overflow: auto;
  overscroll-behavior: contain;
  background: var(--color-sunken);
  border-top: var(--rule-hair) solid var(--color-rule);
  border-bottom: var(--rule-hair) solid var(--color-rule);
}

.png__img {
  display: block;
  margin: 0 auto;
  max-width: none;
}

/* 100% 表示「整幅都在框里」，超过 100% 才是 1:1 放大查看细节 */
.png__img.is-fit {
  max-width: 100%;
  max-height: 100%;
  width: auto;
  height: auto;
  object-fit: contain;
}

.png__zoom {
  display: flex;
  align-items: center;
  gap: var(--space-2xs);
}

.png__zoom-value {
  min-width: 46px;
  text-align: center;
  font-size: var(--text-xs);
  color: var(--color-muted);
}

.png__actions {
  display: flex;
  gap: var(--space-xs);
}
</style>