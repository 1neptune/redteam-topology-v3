import { defineStore } from 'pinia'
import { ref } from 'vue'

export type ToastTone = 'ok' | 'warn' | 'error'

export interface ToastItem {
  id: number
  message: string
  tone: ToastTone
}

export interface ConfirmOptions {
  title: string
  message: string
  confirmText?: string
  cancelText?: string
  danger?: boolean
}

interface ConfirmState extends ConfirmOptions {
  visible: boolean
  resolve: ((value: boolean) => void) | null
}

/** 轻提示与确认框的全局状态 */
export const useUiStore = defineStore('ui', () => {
  const toasts = ref<ToastItem[]>([])
  const confirmState = ref<ConfirmState>({
    visible: false,
    title: '',
    message: '',
    resolve: null,
  })
  let seed = 0

  /** 弹出一条提示 */
  function toast(message: string, tone: ToastTone = 'ok', duration = 3200): void {
    const id = ++seed
    toasts.value = [...toasts.value, { id, message, tone }]
    window.setTimeout(() => dismiss(id), duration)
  }

  /** 关闭指定提示 */
  function dismiss(id: number): void {
    toasts.value = toasts.value.filter((item) => item.id !== id)
  }

  /** 打开确认框，返回用户选择 */
  function confirm(options: ConfirmOptions): Promise<boolean> {
    return new Promise((resolve) => {
      confirmState.value = { ...options, visible: true, resolve }
    })
  }

  /** 关闭确认框并回传结果 */
  function settle(result: boolean): void {
    confirmState.value.resolve?.(result)
    confirmState.value = { visible: false, title: '', message: '', resolve: null }
  }

  return { toasts, confirmState, toast, dismiss, confirm, settle }
})