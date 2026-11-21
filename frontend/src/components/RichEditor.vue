<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { uploadImage } from '@/api/vulnImage'
import { sanitizeHtml } from '@/lib/richText'
import { useUiStore } from '@/stores/ui'

const props = defineProps<{
  modelValue: string
  disabled?: boolean
}>()

const emit = defineEmits<{ 'update:modelValue': [value: string] }>()

const ui = useUiStore()
const editor = ref<HTMLElement | null>(null)
const fileInput = ref<HTMLInputElement | null>(null)
const uploading = ref(false)
const focused = ref(false)

/** 标记本次变化来自编辑框自身，避免回写时把内容再灌一遍、光标跳走 */
let fromSelf = false

/** 把编辑框内容回写给父组件 */
function sync(): void {
  const html = editor.value?.innerHTML ?? ''
  fromSelf = true
  emit('update:modelValue', sanitizeHtml(html))
}

/** 外部值变化时同步进编辑框（首次挂载、切换条目、撤销时） */
function restore(): void {
  if (!editor.value) return
  if (editor.value.innerHTML !== (props.modelValue || '')) {
    editor.value.innerHTML = props.modelValue || ''
  }
}

onMounted(restore)

// 切换漏洞条目或撤销时，父组件的值会变，编辑框要跟着换内容
watch(
  () => props.modelValue,
  () => {
    if (fromSelf) {
      fromSelf = false
      return
    }
    restore()
  },
)

/** 执行一个编辑命令 */
function run(command: string): void {
  editor.value?.focus()
  document.execCommand(command, false)
  sync()
}

/** 插入有序/无序列表 */
function insertList(ordered: boolean): void {
  run(ordered ? 'insertOrderedList' : 'insertUnorderedList')
}

/** 选择图片并内联进正文 */
async function pickImage(event: Event): Promise<void> {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file || !editor.value) return
  uploading.value = true
  try {
    // 正文只存带签名的短地址，图片作为独立资源由浏览器缓存
    const url = await uploadImage(file)
    editor.value.focus()
    document.execCommand('insertImage', false, url)
    sync()
  } catch (err) {
    ui.toast(err instanceof Error ? err.message : '插入图片失败', 'error')
  } finally {
    uploading.value = false
  }
}

/** 粘贴时只取纯文本，挡住外部样式与脚本 */
function onPaste(event: ClipboardEvent): void {
  event.preventDefault()
  const text = event.clipboardData?.getData('text/plain') ?? ''
  document.execCommand('insertText', false, text)
  sync()
}

</script>

<template>
  <div class="rte" :class="{ 'is-focused': focused }">
    <div v-if="!disabled" class="rte__bar">
      <button type="button" class="rte__btn" @click="run('bold')">粗体</button>
      <button type="button" class="rte__btn" @click="run('italic')">斜体</button>
      <button type="button" class="rte__btn" @click="insertList(false)">列表</button>
      <button type="button" class="rte__btn" @click="insertList(true)">编号</button>
      <button type="button" class="rte__btn" :disabled="uploading" @click="fileInput?.click()">
        {{ uploading ? '插入中' : '插入图片' }}
      </button>
      <input ref="fileInput" type="file" accept="image/*" hidden @change="pickImage" />
    </div>

    <div
      ref="editor"
      class="rte__body"
      :contenteditable="!disabled"
      role="textbox"
      aria-multiline="true"
      @input="sync"
      @blur="focused = false; sync()"
      @focus="focused = true"
      @paste="onPaste"
    />
  </div>
</template>

<style scoped>
.rte {
  border: 1px solid var(--color-rule);
  border-radius: var(--radius-card);
  background: var(--color-paper);
  overflow: hidden;
}

.rte.is-focused {
  border-color: var(--color-accent);
}

.rte__bar {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-3xs);
  padding: var(--space-2xs);
  border-bottom: 1px solid var(--color-rule);
  background: var(--color-paper-2);
}

.rte__btn {
  padding: var(--space-3xs) var(--space-2xs);
  border: 1px solid transparent;
  border-radius: var(--radius-card);
  background: transparent;
  color: var(--color-muted);
  font: inherit;
  font-size: 11px;
  cursor: pointer;
}

.rte__btn:hover:not(:disabled) {
  color: var(--color-ink);
  border-color: var(--color-rule-2);
}

.rte__btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.rte__btn:focus-visible {
  outline: 2px solid var(--color-focus);
  outline-offset: 1px;
}

.rte__body {
  min-height: 120px;
  max-height: 320px;
  overflow-y: auto;
  padding: var(--space-xs) var(--space-sm);
  font-size: var(--text-sm);
  line-height: 1.65;
  color: var(--color-ink-2);
  outline: none;
  overflow-wrap: anywhere;
}

.rte__body:empty::before {
  content: '复现步骤、请求报文、验证结果';
  color: var(--color-muted);
}

.rte__body :deep(img) {
  max-width: 100%;
  height: auto;
  display: block;
  margin: var(--space-2xs) 0;
  border-radius: var(--radius-card);
}

.rte__body :deep(ul),
.rte__body :deep(ol) {
  margin: var(--space-2xs) 0;
  padding-left: var(--space-md);
}

.rte__body :deep(code) {
  font-family: var(--font-outlier);
  font-size: 0.95em;
  color: var(--color-cool);
}
</style>