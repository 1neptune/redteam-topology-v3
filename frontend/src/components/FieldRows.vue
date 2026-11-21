<script setup lang="ts">
import { computed, ref } from 'vue'
import type { GraphField } from '@/types'

const props = defineProps<{
  fields: GraphField[]
  disabled?: boolean
}>()
const emit = defineEmits<{ change: [fields: GraphField[]] }>()

const draftKey = ref('')
const draftValue = ref('')

const rows = computed<GraphField[]>(() => props.fields ?? [])

/** 修改已有字段 */
function update(index: number, part: 'key' | 'value', value: string): void {
  emit(
    'change',
    rows.value.map((row, i) => (i === index ? { ...row, [part]: value } : row)),
  )
}

/** 删除字段 */
function remove(index: number): void {
  emit('change', rows.value.filter((_, i) => i !== index))
}

/** 追加草稿里的字段 */
function appendDraft(): void {
  const key = draftKey.value.trim()
  const value = draftValue.value.trim()
  if (!key && !value) return
  emit('change', [...rows.value, { key, value }])
  draftKey.value = ''
  draftValue.value = ''
}
</script>

<template>
  <div class="kv">
    <div v-for="(row, index) in rows" :key="`kv-${index}`" class="kv-row">
      <input
        class="input mono"
        :value="row.key"
        :disabled="disabled"
        placeholder="字段名"
        spellcheck="false"
        @input="update(index, 'key', ($event.target as HTMLInputElement).value)"
      />
      <input
        class="input"
        :value="row.value"
        :disabled="disabled"
        placeholder="值"
        spellcheck="false"
        @input="update(index, 'value', ($event.target as HTMLInputElement).value)"
      />
      <button v-if="!disabled" class="icon-btn" title="删除字段" @click="remove(index)">
        <svg aria-hidden="true" viewBox="0 0 24 24"><path d="M6 6l12 12M18 6L6 18" /></svg>
      </button>
      <span v-else />
    </div>

    <div v-if="!disabled" class="kv-row">
      <input
        v-model="draftKey"
        class="input mono"
        placeholder="新字段"
        spellcheck="false"
        @keydown.enter.prevent="appendDraft"
      />
      <input
        v-model="draftValue"
        class="input"
        placeholder="值"
        spellcheck="false"
        @keydown.enter.prevent="appendDraft"
      />
      <button class="icon-btn" title="添加字段" @click="appendDraft">
        <svg aria-hidden="true" viewBox="0 0 24 24"><path d="M12 5v14M5 12h14" /></svg>
      </button>
    </div>

    <p v-if="!rows.length && disabled" class="kv__empty">未填写自定义字段</p>
  </div>
</template>

<style scoped>
.kv {
  display: grid;
  gap: var(--space-3xs);
}

.kv__empty {
  font-size: 11px;
  color: var(--color-muted);
}
</style>