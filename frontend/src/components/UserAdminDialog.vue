<script setup lang="ts">
import { ref, watch } from 'vue'
import * as userApi from '@/api/user'
import * as projectApi from '@/api/project'
import { useAuthStore } from '@/stores/auth'
import { useUiStore } from '@/stores/ui'
import { formatStamp } from '@/lib/format'
import type { ProjectSummary, User, UserRole } from '@/types'

const props = defineProps<{ visible: boolean }>()
const emit = defineEmits<{ close: [] }>()

const auth = useAuthStore()
const ui = useUiStore()

const users = ref<User[]>([])
const loading = ref(false)
const busy = ref(false)
const DEFAULT_PASSWORD = 'Atlas#2026'
const projects = ref<ProjectSummary[]>([])
const form = ref({
  username: '',
  displayName: '',
  password: DEFAULT_PASSWORD,
  role: 'member' as UserRole,
  projectId: '',
  permission: 'editor',
})

watch(
  () => props.visible,
  (open) => {
    if (open) {
      form.value.password = DEFAULT_PASSWORD
      void load()
      void loadProjects()
    }
  },
)

/** 可选项目（用于建号时直接授权） */
async function loadProjects(): Promise<void> {
  try {
    projects.value = await projectApi.fetchProjects()
  } catch {
    projects.value = []
  }
}

/** 拉取账号列表 */
async function load(): Promise<void> {
  loading.value = true
  try {
    users.value = await userApi.fetchUsers()
  } catch (err) {
    ui.toast(err instanceof Error ? err.message : '账号列表加载失败', 'error')
  } finally {
    loading.value = false
  }
}

/** 新建账号 */
async function create(): Promise<void> {
  if (busy.value) return
  busy.value = true
  try {
    await userApi.createUser({
      username: form.value.username.trim(),
      displayName: form.value.displayName.trim() || form.value.username.trim(),
      password: form.value.password,
      role: form.value.role,
      projectId: form.value.projectId || undefined,
      permission: form.value.projectId ? form.value.permission : undefined,
    })
    form.value = {
      username: '',
      displayName: '',
      password: DEFAULT_PASSWORD,
      role: 'member',
      projectId: '',
      permission: 'editor',
    }
    await load()
    await auth.refreshDirectory()
    ui.toast('账号已创建', 'ok')
  } catch (err) {
    ui.toast(err instanceof Error ? err.message : '创建失败', 'error')
  } finally {
    busy.value = false
  }
}

/** 改角色 */
async function setRole(user: User, role: UserRole): Promise<void> {
  try {
    await userApi.updateUser(user.id, { role })
    await load()
    ui.toast('角色已更新', 'ok')
  } catch (err) {
    ui.toast(err instanceof Error ? err.message : '更新失败', 'error')
  }
}

/** 启停账号 */
async function toggleDisabled(user: User): Promise<void> {
  try {
    await userApi.updateUser(user.id, { disabled: !user.disabled })
    await load()
    ui.toast(user.disabled ? '账号已启用' : '账号已停用', 'ok')
  } catch (err) {
    ui.toast(err instanceof Error ? err.message : '更新失败', 'error')
  }
}

/** 重置口令为随机串并提示转交 */
/** 删除账号 */
async function removeUser(user: User): Promise<void> {
  const ok = await ui.confirm({
    title: '删除账号',
    message: `将删除「${user.displayName}」并移除其在所有项目中的授权。`,
    confirmText: '删除',
    danger: true,
  })
  if (!ok) return
  try {
    await userApi.deleteUser(user.id)
    await load()
    await auth.refreshDirectory()
    ui.toast('账号已删除', 'ok')
  } catch (err) {
    ui.toast(err instanceof Error ? err.message : '删除失败', 'error')
  }
}

async function resetPassword(user: User): Promise<void> {
  const next = DEFAULT_PASSWORD
  const ok = await ui.confirm({
    title: '重置口令',
    message: `将「${user.displayName}」的口令重置为 ${next}，其所有会话会立即失效。`,
    confirmText: '重置',
    danger: true,
  })
  if (!ok) return
  try {
    await userApi.updateUser(user.id, { password: next })
    ui.toast(`新口令：${next}`, 'ok', 9000)
  } catch (err) {
    ui.toast(err instanceof Error ? err.message : '重置失败', 'error')
  }
}
</script>

<template>
  <div v-if="visible" class="modal-mask" @click.self="emit('close')">
    <div class="modal modal--wide" role="dialog" aria-modal="true" aria-label="用户管理">
      <div class="modal__head">用户管理</div>
      <div class="modal__body">
        <div class="new-user">
          <div class="field">
            <label class="field__label" for="nu-name">用户名</label>
            <input id="nu-name" v-model="form.displayName" class="input" placeholder="张三" />
          </div>
          <div class="field">
            <label class="field__label" for="nu-user">账号</label>
            <input
              id="nu-user"
              v-model="form.username"
              class="input mono"
              placeholder="zhangsan"
              spellcheck="false"
              size="5"
            />
          </div>
          <div class="field">
            <label class="field__label" for="nu-pass">初始化</label>
            <input id="nu-pass" v-model="form.password" class="input" placeholder="至少 8 位，含大小写" />
          </div>
          <div class="field">
            <label class="field__label" for="nu-role">角色</label>
            <select id="nu-role" v-model="form.role" class="select">
              <option value="member">组员</option>
              <option value="admin">管理员</option>
            </select>
          </div>
          <div class="field">
            <label class="field__label" for="nu-project">项目</label>
            <select id="nu-project" v-model="form.projectId" class="select">
              <option value="">不授权</option>
              <option v-for="item in projects" :key="item.id" :value="item.id">{{ item.name }}</option>
            </select>
          </div>
          <div class="field">
            <label class="field__label" for="nu-perm">权限</label>
            <select id="nu-perm" v-model="form.permission" class="select" :disabled="!form.projectId">
              <option value="editor">可编辑</option>
              <option value="viewer">只读</option>
              <option value="owner">管理</option>
            </select>
          </div>
          <button class="btn btn--primary new-user__submit" :disabled="busy" @click="create">创建</button>
        </div>

        <div class="user-table">
          <div class="user-row user-row--head">
            <span>成员</span>
            <span>角色</span>
            <span>最近登录</span>
            <span>操作</span>
          </div>
          <div v-for="user in users" :key="user.id" class="user-row">
            <span class="user-row__id">
              <b>{{ user.displayName }}</b>
              <em class="mono">{{ user.username }}</em>
              <span v-if="user.disabled" class="chip chip--bad">已停用</span>
            </span>
            <select
              class="select user-row__role"
              :value="user.role"
              :disabled="user.id === auth.user?.id"
              @change="setRole(user, ($event.target as HTMLSelectElement).value as UserRole)"
            >
              <option value="member">组员</option>
              <option value="admin">管理员</option>
            </select>
            <span class="mono user-row__time">
              {{ formatStamp(user.lastLoginAt) }}
            </span>
            <span class="user-row__ops">
              <button class="btn btn--sm btn--ghost" @click="resetPassword(user)">重置口令</button>
              <button
                class="btn btn--sm"
                :class="user.disabled ? '' : 'btn--danger'"
                @click="toggleDisabled(user)"
              >
                {{ user.disabled ? '启用' : '停用' }}
              </button>
              <button
                class="btn btn--sm btn--danger"
                :disabled="user.id === auth.user?.id"
                @click="removeUser(user)"
              >
                删除
              </button>
            </span>
          </div>
          <p v-if="!users.length && !loading" class="user-hint">还没有账号。</p>
        </div>

      </div>
      <div class="modal__foot">
        <button class="btn" @click="emit('close')">关闭</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 建号表单要容纳项目与权限，表格要放下账号、角色、时间与三个操作 */
/* 全局 .modal 是 width: min(520px, 100%)，这里必须改 width，只写 max-width 会被压回去 */
.modal--wide {
  width: min(880px, 100%);
  max-width: 880px;
}
/* 创建按钮与同一行的下拉框等高同框：高度与圆角都跟控件走 */
.new-user__submit {
  height: var(--control-h-sm);
  padding-inline: var(--space-md);
  border-radius: var(--radius-card);
  font-size: var(--text-xs);
}

.new-user {
  display: grid;
  grid-template-columns: minmax(88px, 1fr) 92px minmax(104px, 1fr) 92px minmax(176px, 1.6fr) 84px auto;
  gap: var(--space-xs);
  align-items: end;
  padding-bottom: var(--space-sm);
  border-bottom: 1px solid var(--color-rule);
}

.user-table {
  display: grid;
}

/* 列宽由内容决定：账号、角色、时间、操作各自按最长文字撑开，表头与内容共用 */
.user-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 92px 128px 232px;
  align-items: center;
  gap: var(--space-sm);
  padding: var(--space-2xs) var(--space-xs);
  border-bottom: 1px solid var(--color-rule);
  font-size: var(--text-xs);
}

/* 表头与内容共用同一套列定义，文字统一左对齐，控件铺满所在列 */
.user-row--head {
  font-size: 11px;
  color: var(--color-muted);
  border-bottom: 1px solid var(--color-rule-2);
}

.user-row--head span,
.user-row > * {
  text-align: left;
  justify-self: stretch;
  min-width: 0;
}

.user-row__role {
  width: 100%;
}

.user-row__id {
  display: flex;
  align-items: center;
  gap: var(--space-xs);
  min-width: 0;
}

.user-row__id b {
  font-weight: 500;
  font-size: var(--text-xs);
  color: var(--color-ink);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.user-row__id em {
  font-style: normal;
  font-size: 10px;
  color: var(--color-muted);
}

.user-row__role {
  height: var(--control-h-sm);
  font-size: 11px;
}

.user-row__time {
  font-size: 10px;
  color: var(--color-cool);
}

.user-row__ops {
  display: flex;
  gap: var(--space-3xs);
  justify-content: flex-end;
}

.user-hint {
  font-size: 11px;
  line-height: 1.6;
  color: var(--color-muted);
}
</style>