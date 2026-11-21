<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'
import { useProjectsStore } from '@/stores/projects'
import { useUiStore } from '@/stores/ui'
import { downloadBlob } from '@/lib/exporter'
import { fileStamp, formatDateTime, formatRelative, safeFilename } from '@/lib/format'
import ChangePasswordDialog from '@/components/ChangePasswordDialog.vue'
import UserAdminDialog from '@/components/UserAdminDialog.vue'
import type { ExportBundle, Graph, ProjectStatus, ProjectSummary } from '@/types'

const router = useRouter()
const auth = useAuthStore()
const store = useProjectsStore()
const ui = useUiStore()

const showCreate = ref(false)
const showUsers = ref(false)
const showPassword = ref(false)
const newName = ref('')
const newDesc = ref('')
const newStatus = ref<ProjectStatus>('active')
const editingId = ref('')
const creating = ref(false)
const fileInput = ref<HTMLInputElement | null>(null)

const PERMISSION_LABEL: Record<string, string> = { owner: '管理', editor: '可编辑', viewer: '只读' }

onMounted(() => {
  void store.refresh()
})

/** 打开新建对话框 */
function openCreate(): void {
  editingId.value = ''
  newName.value = ''
  newDesc.value = ''
  newStatus.value = 'active'
  showCreate.value = true
}

/** 编辑项目资料：复用同一个对话框 */
function openEdit(project: ProjectSummary): void {
  editingId.value = project.id
  newName.value = project.name
  newDesc.value = project.description ?? ''
  newStatus.value = project.status ?? 'active'
  showCreate.value = true
}

/** 关闭对话框并复位 */
function closeEditor(): void {
  showCreate.value = false
  editingId.value = ''
}

/** 提交新建 */
async function submitCreate(): Promise<void> {
  if (creating.value) return
  creating.value = true
  try {
    const name = newName.value.trim() || '未命名项目'
    const description = newDesc.value.trim()
    if (editingId.value) {
      // 编辑：只改资料与状态，不动拓扑
      await store.updateMeta(editingId.value, { name, description, status: newStatus.value })
      closeEditor()
      ui.toast('项目已更新', 'ok')
      return
    }
    const project = await store.create({ name, description, status: newStatus.value })
    showCreate.value = false
    void router.push({ name: 'editor', params: { id: project.id } })
  } catch (err) {
    ui.toast(err instanceof Error ? err.message : '保存失败', 'error')
  } finally {
    creating.value = false
  }
}

/** 打开项目 */
function open(project: ProjectSummary): void {
  void router.push({ name: 'editor', params: { id: project.id } })
}

/** 导出 .atlas.json */
async function exportProject(project: ProjectSummary): Promise<void> {
  try {
    const bundle: ExportBundle = await store.exportBundle(project.id)
    const blob = new Blob([JSON.stringify(bundle, null, 2)], { type: 'application/json;charset=utf-8' })
    downloadBlob(blob, `${safeFilename(project.name)}-${fileStamp()}.atlas.json`)
    ui.toast('已导出 JSON', 'ok')
  } catch (err) {
    ui.toast(err instanceof Error ? err.message : '导出失败', 'error')
  }
}

/** 复制项目 */
async function duplicate(project: ProjectSummary): Promise<void> {
  try {
    await store.duplicate(project.id)
    ui.toast('已复制为新项目', 'ok')
  } catch (err) {
    ui.toast(err instanceof Error ? err.message : '复制失败', 'error')
  }
}

/** 删除项目 */
async function remove(project: ProjectSummary): Promise<void> {
  const ok = await ui.confirm({
    title: '删除项目',
    message: `项目「${project.name}」及其拓扑、审计记录都会被删除，且无法恢复。`,
    confirmText: '删除',
    danger: true,
  })
  if (!ok) return
  try {
    await store.remove(project.id)
    ui.toast('项目已删除', 'ok')
  } catch (err) {
    ui.toast(err instanceof Error ? err.message : '删除失败', 'error')
  }
}

/** 选择导入文件 */
function pickFile(): void {
  fileInput.value?.click()
}

/** 导入 .atlas.json */
async function onImport(event: Event): Promise<void> {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file) return
  try {
    const text = await file.text()
    const parsed = JSON.parse(text) as { project?: { name?: string; description?: string; data?: Graph } }
    const data = parsed.project?.data
    if (!data || !Array.isArray(data.nodes)) {
      throw new Error('文件里没有可用的拓扑数据')
    }
    const created = await store.importFile({
      project: {
        name: parsed.project?.name?.trim() || `导入-${fileStamp()}`,
        description: parsed.project?.description ?? '',
        data,
      },
    })
    ui.toast('导入成功', 'ok')
    void router.push({ name: 'editor', params: { id: created.id } })
  } catch (err) {
    ui.toast(err instanceof Error ? err.message : '导入失败，请检查文件格式', 'error')
  }
}

/** 退出登录 */
async function logout(): Promise<void> {
  await auth.logout()
  void router.replace({ name: 'login' })
}
</script>

<template>
  <div class="app">
    <header class="topbar">
      <a class="brand" href="#/">
        <span class="brand__mark">
          <svg aria-hidden="true" viewBox="0 0 24 24"><path d="M4 18V7l6 7 3-4 5 8" /></svg>
        </span>
        <span class="brand__text">RedTeam <span>Atlas</span></span>
      </a>

      <span class="topbar__spacer" />

      <div class="topbar__meta">
        <span v-if="auth.isAdmin" class="chip chip--person">管理员</span>
        <span v-else class="chip">{{ auth.displayName }}</span>
      </div>

      <button class="btn btn--sm btn--ghost" @click="showPassword = true">修改口令</button>
      <button v-if="auth.isAdmin" class="btn btn--sm btn--ghost" @click="showUsers = true">用户管理</button>
      <button class="btn btn--sm btn--ghost" @click="logout">退出</button>
    </header>

    <div class="app__main">
      <div class="page">
        <div class="page__head">
          <div>
            <h1 class="page__title">项目</h1>
          </div>

          <div class="page__actions">
            <div class="search">
              <svg aria-hidden="true" viewBox="0 0 24 24"><circle cx="11" cy="11" r="7" /><path d="m20 20-3.2-3.2" /></svg>
              <input
                class="input"
                type="search"
                placeholder="搜索项目"
                aria-label="搜索项目"
                :value="store.query"
                @input="store.setQuery(($event.target as HTMLInputElement).value)"
              />
            </div>
            <button class="btn btn--sm" @click="pickFile">导入</button>
            <input ref="fileInput" type="file" accept="application/json,.json" hidden @change="onImport" />
            <button class="btn btn--sm btn--primary" @click="openCreate">新建项目</button>
          </div>
        </div>

        <div v-if="store.loading" class="panel">
          <div class="panel__head"><span class="panel__title">正在读取项目</span></div>
          <div class="panel__body skeleton">
            <span v-for="i in 5" :key="i" style="height: 34px" />
          </div>
        </div>

        <div v-else-if="store.error" class="state state--error">
          <h3>项目列表加载失败</h3>
          <p>{{ store.error }}</p>
          <button class="btn btn--sm" @click="store.refresh()">重试</button>
        </div>

        <div v-else-if="store.isEmpty" class="state">
          <h3>{{ store.query ? '没有匹配的项目' : '还没有项目' }}</h3>
          <p>
            {{
              store.query
                ? '换一个关键词，或清空搜索框。'
                : '新建一个项目开始记录攻击路径。'
            }}
          </p>
          <button v-if="!store.query" class="btn btn--primary btn--sm" @click="openCreate">新建项目</button>
        </div>

        <div v-else class="panel">
          <table class="table">
            <thead>
              <tr>
                <th scope="col">项目</th>
                <th scope="col" style="width: 88px">状态</th>
                <th scope="col" style="width: 80px">资产</th>
                <th scope="col" style="width: 80px">链路</th>
                <th scope="col" style="width: 96px">我的权限</th>
                <th scope="col" style="width: 190px">最近更新</th>
                <th scope="col" style="width: 140px"></th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="project in store.projects" :key="project.id" @click="open(project)">
                <td>
                  <div class="project-name">{{ project.name }}</div>
                  <div class="project-desc">{{ project.description || '无说明' }}</div>
                </td>
                <td>
                  <span class="chip" :class="project.status === 'done' ? 'chip--ok' : 'chip--system'">
                    {{ project.status === 'done' ? '已完成' : '进行中' }}
                  </span>
                </td>
                <td class="num">{{ project.nodeCount }}</td>
                <td class="num">{{ project.edgeCount }}</td>
                <td>
                  <span class="chip" :class="project.permission === 'owner' ? 'chip--person' : 'chip--system'">
                    {{ PERMISSION_LABEL[project.permission] ?? project.permission }}
                  </span>
                </td>
                <td>
                  <div class="update-cell">
                    <span class="update-cell__who">{{ project.updatedBy || '—' }}</span>
                    <span class="update-cell__when mono" :title="formatRelative(project.updatedAt)">{{ formatDateTime(project.updatedAt) }}</span>
                  </div>
                </td>
                <td>
                  <div class="row-ops" @click.stop>
                    <button class="btn btn--sm btn--ghost" @click="exportProject(project)">导出</button>
                    <button class="btn btn--sm btn--ghost" @click="openEdit(project)">编辑</button>
                    <button class="btn btn--sm btn--ghost" @click="duplicate(project)">复制</button>
                    <button
                      v-if="project.permission === 'owner'"
                      class="btn btn--sm btn--ghost"
                      @click="remove(project)"
                    >
                      删除
                    </button>
                  </div>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>

    <div v-if="showCreate" class="modal-mask" @click.self="showCreate = false">
      <div class="modal" role="dialog" aria-modal="true">
        <div class="modal__head">{{ editingId ? '编辑项目' : '新建项目' }}</div>
        <div class="modal__body">
          <div class="field">
            <label class="field__label" for="p-name">项目名称</label>
            <input
              id="p-name"
              v-model="newName"
              class="input"
              placeholder="2026 某集团攻防演练"
              @keydown.enter="submitCreate"
            />
          </div>
          <div class="field">
            <label class="field__label" for="p-desc">说明</label>
            <textarea id="p-desc" v-model="newDesc" class="textarea" placeholder="目标范围、时间窗口、约束条件" />
          </div>
          <div class="field">
            <label class="field__label" for="p-status">项目状态</label>
            <select id="p-status" v-model="newStatus" class="select">
              <option value="active">进行中</option>
              <option value="done">已完成</option>
            </select>
          </div>
        </div>
        <div class="modal__foot">
          <button class="btn" @click="closeEditor">取消</button>
          <button class="btn btn--primary" :disabled="creating" @click="submitCreate">
            {{ creating ? '保存中…' : editingId ? '保存' : '创建' }}
          </button>
        </div>
      </div>
    </div>

    <UserAdminDialog :visible="showUsers" @close="showUsers = false" />
    <ChangePasswordDialog :visible="showPassword" @close="showPassword = false" />
  </div>
</template>

<style scoped>
/* 项目名按文字长度自然撑开：不换行、不省略；说明仍限宽，避免整行被拉长 */
.project-name {
  font-size: var(--text-sm);
  color: var(--color-ink);
  font-weight: 500;
  white-space: nowrap;
  overflow-wrap: normal;
}

.table {
  table-layout: auto;
}

.project-desc {
  font-size: 11px;
  color: var(--color-muted);
  margin-top: 1px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 46ch;
}

.update-cell {
  display: grid;
  gap: 1px;
}

.update-cell__who {
  font-size: 11px;
  color: var(--color-accent);
}

.update-cell__when {
  font-size: 10px;
  color: var(--color-cool);
}

.row-ops {
  display: flex;
  gap: 2px;
  justify-content: flex-end;
  opacity: 0.55;
  transition: opacity var(--dur-micro) var(--ease-out);
}

tbody tr:hover .row-ops,
tbody tr:focus-within .row-ops {
  opacity: 1;
}

@media (max-width: 60rem) {
  .row-ops {
    opacity: 1;
  }
}
</style>