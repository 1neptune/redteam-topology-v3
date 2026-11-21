<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useCatalogStore } from '@/stores/catalog'
import { useUiStore } from '@/stores/ui'
import { iconOf } from '@/lib/nodeTypes'
import { BUILTIN_ICONS, builtinUrl, randomBuiltinUrl } from '@/lib/iconSet'
import TypeIcon from '@/components/TypeIcon.vue'
import type { AssetType } from '@/types'

const props = defineProps<{ visible: boolean }>()
const emit = defineEmits<{ close: [] }>()

const catalog = useCatalogStore()
const ui = useUiStore()

const label = ref('')
const color = ref('#d9a441')
const iconUrl = ref(randomBuiltinUrl())
const uploading = ref(false)
const saving = ref(false)
const editingSlug = ref('')

const custom = computed(() => catalog.customTypes)
const canSubmit = computed(() => label.value.trim().length > 0)

watch(
  () => props.visible,
  (open) => {
    if (!open) return
    resetForm()
    void catalog.load()
  },
)

/** 清空表单 */
function resetForm(): void {
  label.value = ''
  color.value = '#d9a441'
  // 新建时随机预选一个内置图标，用户可换或上传替换
  iconUrl.value = randomBuiltinUrl()
  editingSlug.value = ''
}

/** 上传图标 */
async function onPickIcon(event: Event): Promise<void> {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file) return
  uploading.value = true
  try {
    iconUrl.value = await catalog.uploadIcon(file)
    ui.toast('图标已上传', 'ok')
  } catch (err) {
    ui.toast(err instanceof Error ? err.message : '图标上传失败', 'error')
  } finally {
    uploading.value = false
  }
}

/** 保存（新建或更新） */
async function submit(): Promise<void> {
  if (!canSubmit.value || saving.value) return
  saving.value = true
  try {
    if (editingSlug.value) {
      await catalog.update(editingSlug.value, {
        label: label.value.trim(),
        color: color.value,
        iconUrl: iconUrl.value || null,
      })
      ui.toast('组件已更新', 'ok')
    } else {
      await catalog.create({
        label: label.value.trim(),
        color: color.value,
        iconUrl: iconUrl.value || null,
      })
      ui.toast('组件已创建', 'ok')
    }
    resetForm()
  } catch (err) {
    ui.toast(err instanceof Error ? err.message : '保存失败', 'error')
  } finally {
    saving.value = false
  }
}

/** 进入编辑态 */
function edit(type: AssetType): void {
  editingSlug.value = type.slug
  label.value = type.label
  color.value = type.color
  iconUrl.value = type.iconUrl ?? randomBuiltinUrl()
}

/** 删除自定义组件 */
async function remove(type: AssetType): Promise<void> {
  const ok = await ui.confirm({
    title: '删除自定义组件',
    message: `删除「${type.label}」后，已使用它的节点会回退为内置外观。`,
    confirmText: '删除',
    danger: true,
  })
  if (!ok) return
  try {
    await catalog.remove(type.slug)
    if (editingSlug.value === type.slug) resetForm()
    ui.toast('组件已删除', 'ok')
  } catch (err) {
    ui.toast(err instanceof Error ? err.message : '删除失败', 'error')
  }
}
</script>

<template>
  <div v-if="visible" class="modal-mask" @click.self="emit('close')">
    <div class="modal" role="dialog" aria-modal="true" aria-label="自定义组件">
      <div class="modal__head">自定义组件</div>
      <div class="modal__body">
        <div class="type-form">
          <div class="field">
            <label class="field__label" for="type-label">组件名称</label>
            <input id="type-label" v-model="label" class="input" placeholder="例如 堡垒机、蜜罐、VPN 网关" />
          </div>

          <div class="field-row">
            <div class="field">
              <label class="field__label" for="type-color">标识色</label>
              <div class="color-row">
                <input id="type-color" v-model="color" class="color-input" type="color" />
                <input v-model="color" class="input mono" spellcheck="false" />
              </div>
            </div>
            <div class="field">
              <span class="field__label">图标</span>
              <div class="icon-pick">
                <button
                  v-for="icon in BUILTIN_ICONS"
                  :key="icon.key"
                  type="button"
                  class="icon-pick__item"
                  :class="{ 'is-on': iconUrl === builtinUrl(icon.key) }"
                  :aria-pressed="iconUrl === builtinUrl(icon.key)"
                  :title="icon.label"
                  @click="iconUrl = builtinUrl(icon.key)"
                >
                  <svg aria-hidden="true" viewBox="0 0 24 24"><path :d="icon.path" /></svg>
                </button>
                <label class="icon-pick__upload" title="上传自定义图标">
                  <TypeIcon :icon-url="iconUrl" :size="14">
                    <svg aria-hidden="true" viewBox="0 0 24 24"><path :d="iconOf('host')" /></svg>
                  </TypeIcon>
                  <span>{{ uploading ? '上传中' : '上传' }}</span>
                  <input
                    type="file"
                    accept="image/png,image/jpeg,image/webp,image/gif,image/svg+xml"
                    hidden
                    @change="onPickIcon"
                  />
                </label>
              </div>
            </div>
          </div>

          <div class="type-form__actions">
            <button class="btn btn--sm btn--primary" :disabled="!canSubmit || saving" @click="submit">
              {{ editingSlug ? '保存修改' : '新增组件' }}
            </button>
            <button v-if="editingSlug" class="btn btn--sm" @click="resetForm">取消编辑</button>
          </div>
        </div>

        <div class="type-list">
          <div class="field__label">已创建（{{ custom.length }}）</div>
          <div v-for="type in custom" :key="type.slug" class="type-row">
            <span class="type-row__swatch" :style="{ background: type.color }">
              <TypeIcon :icon-url="type.iconUrl" :size="12">
                <svg aria-hidden="true" viewBox="0 0 24 24"><path :d="iconOf(type.slug)" /></svg>
              </TypeIcon>
            </span>
            <span class="type-row__name">{{ type.label }}</span>
            <span class="mono type-row__slug">{{ type.slug }}</span>
            <button class="btn btn--sm btn--ghost" @click="edit(type)">编辑</button>
            <button class="btn btn--sm btn--danger" @click="remove(type)">删除</button>
          </div>
          <p v-if="!custom.length" class="type-empty">
            还没有自定义组件
          </p>
        </div>
      </div>
      <div class="modal__foot">
        <button class="btn" @click="emit('close')">关闭</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.type-form {
  display: grid;
  gap: var(--space-sm);
  padding-bottom: var(--space-sm);
  border-bottom: 1px solid var(--color-rule);
}

.type-form__actions {
  display: flex;
  gap: var(--space-xs);
}

.color-row {
  display: grid;
  grid-template-columns: 34px minmax(0, 1fr);
  gap: var(--space-xs);
}

.color-input {
  width: 34px;
  height: var(--control-h);
  padding: 2px;
  border: 1px solid var(--color-rule);
  border-radius: var(--radius-pill);
  background: var(--color-paper-3);
  cursor: pointer;
}

.icon-row {
  display: flex;
  align-items: center;
  gap: var(--space-xs);
}

.icon-preview {
  width: var(--control-h);
  height: var(--control-h);
  display: grid;
  place-items: center;
  border: 1px solid var(--color-rule);
  border-radius: var(--radius-pill);
  background: var(--color-paper-3);
}

.icon-preview img {
  width: 18px;
  height: 18px;
  object-fit: contain;
}

.icon-preview svg {
  width: 14px;
  height: 14px;
  fill: none;
  stroke: var(--type);
  stroke-width: 1.7;
}

.type-list {
  display: grid;
  gap: var(--space-3xs);
}

.type-row {
  display: grid;
  grid-template-columns: 20px minmax(0, 1fr) auto auto auto;
  gap: var(--space-xs);
  align-items: center;
  padding: var(--space-2xs) 0;
  border-bottom: 1px solid color-mix(in oklab, var(--color-rule) 55%, transparent);
}

.type-row__swatch {
  width: 16px;
  height: 16px;
  border-radius: 3px;
  display: grid;
  place-items: center;
}

.type-row__swatch img {
  width: 11px;
  height: 11px;
  object-fit: contain;
}

.type-row__name {
  font-size: var(--text-xs);
  color: var(--color-ink);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.type-row__slug {
  font-size: 10px;
  color: var(--color-muted);
}

.type-empty {
  font-size: 11px;
  color: var(--color-muted);
  line-height: 1.6;
}
.icon-pick {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--space-2xs);
}

.icon-pick__item {
  width: 30px;
  height: 30px;
  padding: 0;
  display: grid;
  place-items: center;
  border: 1px solid var(--color-rule);
  border-radius: var(--radius-pill);
  background: var(--color-paper-3);
  color: var(--color-muted);
  cursor: pointer;
}

.icon-pick__item svg {
  width: 15px;
  height: 15px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.6;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.icon-pick__item:hover {
  color: var(--color-ink);
  border-color: var(--color-rule-2);
}

.icon-pick__item.is-on {
  color: var(--color-accent-ink);
  background: var(--color-accent);
  border-color: var(--color-accent);
}

.icon-pick__item:focus-visible,
.icon-pick__upload:focus-visible {
  outline: 2px solid var(--color-focus);
  outline-offset: 2px;
}

.icon-pick__upload {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2xs);
  height: 30px;
  padding: 0 var(--space-sm);
  border: 1px dashed var(--color-rule-2);
  border-radius: var(--radius-pill);
  color: var(--color-muted);
  font-size: var(--text-xs);
  cursor: pointer;
}

.icon-pick__upload:hover {
  color: var(--color-ink);
  border-color: var(--color-accent);
}


</style>