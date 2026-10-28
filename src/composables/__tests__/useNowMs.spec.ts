// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, defineComponent, h } from 'vue'
import type { App, Ref } from 'vue'
import { useNowMs } from '@/composables/useNowMs'

const START = 1_700_000_000_000
let mounted: { app: App; host: HTMLDivElement }[]

function mountClock(): Ref<number> {
  let now!: Ref<number>
  const host = document.createElement('div')
  document.body.append(host)
  const app = createApp(defineComponent({
    setup() {
      now = useNowMs() as Ref<number>
      return () => h('div')
    },
  }))
  app.mount(host)
  mounted.push({ app, host })
  return now
}

beforeEach(() => {
  vi.useFakeTimers()
  vi.setSystemTime(START)
  mounted = []
})

afterEach(() => {
  for (const { app, host } of mounted) {
    app.unmount()
    host.remove()
  }
  vi.useRealTimers()
})

describe('useNowMs', () => {
  it('初始读取墙钟，每秒重新读取而不是盲目累加', () => {
    const now = mountClock()
    expect(now.value).toBe(START)

    vi.advanceTimersByTime(999)
    expect(now.value).toBe(START)
    vi.advanceTimersByTime(1)
    expect(now.value).toBe(START + 1_000)

    // 模拟系统时间跳变：下一拍必须跟随 Date.now，而非 now += 1
    vi.setSystemTime(START + 9_000)
    vi.advanceTimersByTime(1_000)
    expect(now.value).toBe(START + 10_000)
  })

  it('卸载清理 interval', () => {
    mountClock()
    expect(vi.getTimerCount()).toBe(1)
    mounted[0].app.unmount()
    mounted = []
    expect(vi.getTimerCount()).toBe(0)
  })

  it('多个实例独立持有和清理各自 interval', () => {
    const first = mountClock()
    const second = mountClock()
    expect(vi.getTimerCount()).toBe(2)

    mounted[0].app.unmount()
    mounted = mounted.slice(1)
    expect(vi.getTimerCount()).toBe(1)
    vi.advanceTimersByTime(1_000)
    expect(first.value).toBe(START)
    expect(second.value).toBe(START + 1_000)
  })
})
