import { onScopeDispose, ref } from 'vue'
import type { Ref } from 'vue'

export interface TransientValue<T> {
  value: Readonly<Ref<T>>
  show: (next: T) => void
  clear: () => void
}

/**
 * 在指定时长内持有一个临时值，随后恢复 initialValue。
 *
 * 只管理反馈状态与 timer，不封装产生反馈的业务动作；新值覆盖旧值时取消旧 timer，
 * scope 销毁后不再写状态或创建新 timer。timer 句柄保持普通变量，不进响应式系统。
 */
export function useTransientValue<T>(initialValue: T, durationMs: number): TransientValue<T> {
  const value = ref(initialValue) as Ref<T>
  let timer: ReturnType<typeof setTimeout> | null = null
  let disposed = false

  function cancelTimer(): void {
    if (!timer) return
    clearTimeout(timer)
    timer = null
  }

  function clear(): void {
    if (disposed) return
    cancelTimer()
    value.value = initialValue
  }

  function show(next: T): void {
    if (disposed) return
    cancelTimer()
    value.value = next
    timer = setTimeout(() => {
      value.value = initialValue
      timer = null
    }, durationMs)
  }

  onScopeDispose(() => {
    disposed = true
    cancelTimer()
  })

  return { value, show, clear }
}
