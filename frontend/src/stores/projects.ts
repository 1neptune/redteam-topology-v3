import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import * as projectApi from '@/api/project'
import type {
  CreateProjectPayload,
  ExportBundle,
  ImportPayload,
  ProjectSummary,
  UpdateProjectPayload,
} from '@/types'

/** 项目列表与搜索 */
export const useProjectsStore = defineStore('projects', () => {
  const projects = ref<ProjectSummary[]>([])
  const query = ref('')
  const loading = ref(false)
  const error = ref('')

  const isEmpty = computed(() => !loading.value && !error.value && projects.value.length === 0)
  const totalNodes = computed(() => projects.value.reduce((sum, p) => sum + p.nodeCount, 0))
  const totalEdges = computed(() => projects.value.reduce((sum, p) => sum + p.edgeCount, 0))

  let timer = 0

  /**
   * 拉取列表（服务端搜索）。
   * 返回 Promise 便于调用方等待，不吞掉错误。
   */
  async function refresh(): Promise<void> {
    loading.value = true
    error.value = ''
    try {
      projects.value = await projectApi.fetchProjects(query.value.trim())
    } catch (err) {
      projects.value = []
      error.value = err instanceof Error ? err.message : '项目列表加载失败'
    } finally {
      loading.value = false
    }
  }

  /** 关键字变化：防抖 240ms 后请求 */
  function setQuery(value: string): void {
    query.value = value
    window.clearTimeout(timer)
    timer = window.setTimeout(() => {
      void refresh()
    }, 240)
  }

  /** 改项目资料（名称、说明、状态） */
  async function updateMeta(id: string, payload: UpdateProjectPayload): Promise<ProjectSummary> {
    const updated = await projectApi.patchProject(id, payload)
    const index = projects.value.findIndex((item) => item.id === id)
    if (index >= 0) projects.value[index] = updated
    return updated
  }

  async function create(payload: CreateProjectPayload): Promise<ProjectSummary> {
    const created = await projectApi.createProject(payload)
    await refresh()
    return created
  }

  async function update(id: string, name: string, description: string): Promise<void> {
    await projectApi.patchProject(id, { name, description })
    await refresh()
  }

  async function remove(id: string): Promise<void> {
    await projectApi.deleteProject(id)
    await refresh()
  }

  async function duplicate(id: string): Promise<void> {
    await projectApi.duplicateProject(id)
    await refresh()
  }

  async function importFile(payload: ImportPayload): Promise<ProjectSummary> {
    const created = await projectApi.importProject(payload)
    await refresh()
    return created
  }

  async function exportBundle(id: string): Promise<ExportBundle> {
    return projectApi.exportProject(id)
  }

  /** 切换项目后重置列表状态 */
  function reset(): void {
    projects.value = []
    query.value = ''
    error.value = ''
  }

  return {
    projects,
    query,
    loading,
    error,
    isEmpty,
    totalNodes,
    totalEdges,
    refresh,
    setQuery,
    updateMeta,
    create,
    update,
    remove,
    duplicate,
    importFile,
    exportBundle,
    reset,
  }
})