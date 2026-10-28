import { onMounted, onScopeDispose, ref } from 'vue'
import type { Ref } from 'vue'

/**
 * 当前墙钟（epoch 毫秒）：setup 时读取一次，mounted 后每秒重新读取。
 *
 * 不做 now += 1，系统时间调整或后台定时器节流后能在下一拍追上真实时间。
 * 每个调用方拥有独立 interval；scope 销毁时统一清理。
 */
export function useNowMs(): Readonly<Ref<number>> {
  const nowMs = ref(Date.now())
  let timer: ReturnType<typeof setInterval> | null = null

  onMounted(() => {
    timer = setInterval(() => {
      nowMs.value = Date.now()
    }, 1_000)
  })

  onScopeDispose(() => {
    if (timer) clearInterval(timer)
    timer = null
  })

  return nowMs
}
