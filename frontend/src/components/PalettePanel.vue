<script setup lang="ts">
import { computed, ref } from 'vue'
import { useCatalogStore } from '@/stores/catalog'
import { useEditorStore } from '@/stores/editor'
import { groupTypes, iconOf } from '@/lib/nodeTypes'
import TypeIcon from '@/components/TypeIcon.vue'

const emit = defineEmits<{ add: [slug: string]; manage: [] }>()

const catalog = useCatalogStore()
const editor = useEditorStore()
/** 只读用户不能让组件落到画布上 */
const canCreate = computed(() => editor.canEdit)
const keyword = ref('')

/** 过滤后的分组：组件名、slug、分组名都参与匹配 */
const groups = computed(() => {
  const all = groupTypes(catalog.types)
  const kw = keyword.value.trim().toLowerCase()
  if (!kw) return all
  return all
    .map((group) => ({
      group: group.group,
      items: group.items.filter(
        (item) =>
          item.label.toLowerCase().includes(kw) ||
          item.slug.toLowerCase().includes(kw) ||
          group.group.toLowerCase().includes(kw),
      ),
    }))
    .filter((group) => group.items.length > 0)
})

const total = computed(() => groups.value.reduce((sum, group) => sum + group.items.length, 0))

/** 拖拽时把组件 slug 写入 dataTransfer */
function onDragStart(event: DragEvent, slug: string): void {
  if (!event.dataTransfer) return
  event.dataTransfer.setData('application/atlas-node', slug)
  event.dataTransfer.setData('text/plain', slug)
  event.dataTransfer.effectAllowed = 'copy'
}
</script>

<template>
  <aside class="rail" aria-label="资产组件库">
    <div class="rail__head">
      <div class="rail__title">
        <span>资产组件</span>
        <span class="mono">{{ total }}</span>
      </div>
      <div class="search">
        <svg aria-hidden="true" viewBox="0 0 24 24"><circle cx="11" cy="11" r="7" /><path d="m20 20-3.2-3.2" /></svg>
        <input
          v-model="keyword"
          class="input"
          type="search"
          placeholder="搜索组件"
          aria-label="搜索组件"
          spellcheck="false"
        />
      </div>
    </div>

    <div class="rail__list">
      <template v-for="group in groups" :key="group.group">
        <div class="rail__group">{{ group.group }}</div>
        <button
          v-for="item in group.items"
          :key="item.slug"
          class="palette-item"
          :style="{ color: item.color }"
          :draggable="canCreate" :class="{ 'is-disabled': !canCreate }"
          :title="`拖拽到画布：${item.label}`"
          @dragstart="canCreate && onDragStart($event, item.slug)"
          @click="canCreate && emit('add', item.slug)"
        >
          <span class="palette-item__swatch">
            <TypeIcon :icon-url="item.iconUrl" :size="13">
              <svg aria-hidden="true" viewBox="0 0 24 24"><path :d="iconOf(item.slug)" /></svg>
            </TypeIcon>
          </span>
          <span class="palette-item__label">{{ item.label }}</span>
        </button>
      </template>

      <p v-if="!groups.length" class="rail__empty">没有匹配的组件</p>
    </div>

    <div class="rail__foot">
      <button class="btn btn--sm btn--block" @click="emit('manage')">管理自定义组件</button>
    </div>
  </aside>
</template>

<style scoped>
/* 只读用户看到组件库但拖不动，视觉上明确不可用 */
.palette-item.is-disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.palette-item.is-disabled:hover {
  background: transparent;
}

.rail__empty {
  padding: var(--space-md) 0;
  text-align: center;
  font-size: var(--text-xs);
  color: var(--color-muted);
}

.palette-item__label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}


</style>