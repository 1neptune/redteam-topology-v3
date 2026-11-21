<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import * as projectApi from '@/api/project'
import { useAuthStore } from '@/stores/auth'
import { useEditorStore } from '@/stores/editor'
import { useUiStore } from '@/stores/ui'
import type { MemberEntry, Permission } from '@/types'

const props = defineProps<{ visible: boolean }>()
const emit = defineEmits<{ close: [] }>()

const auth = useAuthStore()
const editor = useEditorStore()
const ui = useUiStore()

const selectedUserId = ref('')
const permission = ref<Permission>('editor')
const busy = ref(false)

/** 可选用户：排除已授权成员 */
const candidates = computed(() => {
  const taken = new Set(editor.members.map((member: MemberEntry) => member.userId))
  return auth.directory.filter((user) => !taken.has(user.id))
})

const ownerCount = computed(() => editor.members.filter((m: MemberEntry) => m.permission === 'owner').length)

watch(
  () => props.visible,
  async (open) => {
    if (!open) return
    selectedUserId.value = ''
    permission.value = 'editor'
    await editor.loadMembers()
    if (!auth.directory.length) await auth.refreshDirectory()
  },
)

/** 授权 */
async function grant(): Promise<void> {
  if (!selectedUserId.value || !editor.project) return
  busy.value = true
  try {
    editor.members = await projectApi.grantMember(editor.project.id, {
      userId: selectedUserId.value,
      permission: permission.value,
    })
    selectedUserId.value = ''
    ui.toast('授权已生效', 'ok')
  } catch (err) {
    ui.toast(err instanceof Error ? err.message : '授权失败', 'error')
  } finally {
    busy.value = false
  }
}

/** 撤销 */
async function revoke(userId: string, name: string): Promise<void> {
  if (!editor.project) return
  const ok = await ui.confirm({
    title: '撤销授权',
    message: `撤销后「${name}」将无法再访问该项目。`,
    confirmText: '撤销',
    danger: true,
  })
  if (!ok) return
  try {
    editor.members = await projectApi.revokeMember(editor.project.id, userId)
    ui.toast('已撤销授权', 'ok')
  } catch (err) {
    ui.toast(err instanceof Error ? err.message : '撤销失败', 'error')
  }
}

/** 调整权限 */
async function changePermission(userId: string, next: Permission): Promise<void> {
  if (!editor.project) return
  try {
    editor.members = await projectApi.grantMember(editor.project.id, { userId, permission: next })
    ui.toast('权限已更新', 'ok')
  } catch (err) {
    ui.toast(err instanceof Error ? err.message : '更新失败', 'error')
  }
}
</script>

<template>
  <div v-if="visible" class="modal-mask" @click.self="emit('close')">
    <div class="modal" role="dialog" aria-modal="true" aria-label="成员授权">
      <div class="modal__head">成员授权</div>
      <div class="modal__body">
        <div class="grant-form">
          <div class="field">
            <label class="field__label" for="grant-user">选择组员</label>
            <select id="grant-user" v-model="selectedUserId" class="select">
              <option value="">从成员名录选择…</option>
              <option v-for="user in candidates" :key="user.id" :value="user.id">
                {{ user.displayName }}（{{ user.username }}）
              </option>
            </select>
          </div>
          <div class="field">
            <label class="field__label" for="grant-permission">权限</label>
            <select id="grant-permission" v-model="permission" class="select">
              <option value="viewer">只读：可查看拓扑</option>
              <option value="editor">可编辑：可修改拓扑</option>
              <option value="owner">管理员：可授权与改资料</option>
            </select>
          </div>
          <button class="btn btn--primary btn--sm" :disabled="!selectedUserId || busy" @click="grant">
            授权
          </button>
        </div>

        <div class="member-table">
          <div class="field__label">当前成员（{{ editor.members.length }}）</div>
          <div v-for="member in editor.members" :key="member.userId" class="member-row">
            <span class="member-row__name">{{ member.displayName }}</span>
            <span class="mono member-row__user">{{ member.username }}</span>
            <select
              class="select member-row__perm"
              :value="member.permission"
              :disabled="member.permission === 'owner' && ownerCount <= 1"
              @change="changePermission(member.userId, ($event.target as HTMLSelectElement).value as Permission)"
            >
              <option value="viewer">只读</option>
              <option value="editor">可编辑</option>
              <option value="owner">管理员</option>
            </select>
            <button class="btn btn--sm btn--danger" @click="revoke(member.userId, member.displayName)">
              撤销
            </button>
          </div>
        </div>
      </div>
      <div class="modal__foot">
        <button class="btn" @click="emit('close')">关闭</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.grant-form {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) auto;
  gap: var(--space-xs);
  align-items: end;
  padding-bottom: var(--space-sm);
  border-bottom: 1px solid var(--color-rule);
}

.member-table {
  display: grid;
  gap: var(--space-3xs);
}

.member-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto 96px auto;
  gap: var(--space-xs);
  align-items: center;
  padding: var(--space-2xs) 0;
  border-bottom: 1px solid color-mix(in oklab, var(--color-rule) 55%, transparent);
}

.member-row__name {
  font-size: var(--text-xs);
  color: var(--color-ink);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.member-row__user {
  font-size: 10px;
  color: var(--color-muted);
}

.member-row__perm {
  height: var(--control-h-sm);
  font-size: 11px;
}

</style>