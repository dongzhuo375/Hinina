import { describe, expect, it, vi } from 'vitest'
import { createPoller, planPollDelayMs } from '@/utils/polling'
import type { PollerOptions } from '@/utils/polling'

/// 榜单轮询节奏：10s ± 2s（HOJ 榜单文档 §9.9 要求间隔 ≥10s）
const INTERVAL = 10_000
const JITTER = 2_000

const rand0 = () => 0
const randHalf = () => 0.5
/// Math.random 取不到 1，此处用 1 验证数学上界
const rand1 = () => 1

/**
 * 可控假时钟：自己维护定时器队列，advance 按到期顺序触发。
 * 相比 vi.useFakeTimers，队列长度（pending）可直接断言，
 * 便于验证「start 幂等只有一个句柄」「stop 清理句柄」。
 */
class FakeClock {
  nowMs = 0
  private timers: Array<{ id: number; handler: () => void; fireAt: number }> = []
  private nextId = 1

  /// 当前待触发的定时器数量
  get pending(): number {
    return this.timers.length
  }

  setTimeoutFn = (handler: () => void, ms: number): ReturnType<typeof setTimeout> => {
    const id = this.nextId++
    this.timers.push({ id, handler, fireAt: this.nowMs + ms })
    return id as unknown as ReturnType<typeof setTimeout>
  }

  clearTimeoutFn = (handle: ReturnType<typeof setTimeout>): void => {
    const id = handle as unknown as number
    this.timers = this.timers.filter((t) => t.id !== id)
  }

  /// 推进时间；期间新排入且到期的定时器同样在本次推进内触发（模拟递归 setTimeout）
  advance(ms: number): void {
    const target = this.nowMs + ms
    for (;;) {
      const due = this.timers
        .filter((t) => t.fireAt <= target)
        .sort((a, b) => a.fireAt - b.fireAt || a.id - b.id)[0]
      if (!due) break
      this.timers = this.timers.filter((t) => t.id !== due.id)
      this.nowMs = due.fireAt
      due.handler()
    }
    this.nowMs = target
  }
}

/// 冲刷微任务与宏任务队列，让 task 返回的 Promise 完成 settle
const flushAsync = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0))

/// 构造注入假时钟的轮询器
function makePoller(
  clock: FakeClock,
  overrides: Partial<PollerOptions> = {},
): ReturnType<typeof createPoller> {
  return createPoller({
    task: () => {},
    intervalMs: INTERVAL,
    setTimeoutFn: clock.setTimeoutFn,
    clearTimeoutFn: clock.clearTimeoutFn,
    ...overrides,
  })
}

describe('planPollDelayMs — 间隔 ± 抖动', () => {
  it('rand=0 取下界 interval - jitter，rand=1 取上界 interval + jitter', () => {
    expect(planPollDelayMs(INTERVAL, JITTER, rand0)).toBe(INTERVAL - JITTER)
    expect(planPollDelayMs(INTERVAL, JITTER, rand1)).toBe(INTERVAL + JITTER)
  })

  it('rand=0.5 时抖动抵消，恰为 interval', () => {
    expect(planPollDelayMs(INTERVAL, JITTER, randHalf)).toBe(INTERVAL)
  })

  it('jitter 为 0 时恒等于 interval', () => {
    expect(planPollDelayMs(INTERVAL, 0, rand0)).toBe(INTERVAL)
    expect(planPollDelayMs(INTERVAL, 0, rand1)).toBe(INTERVAL)
  })

  it('抖动大于间隔时钳到 0，结果不得为负（负延迟会退化为忙轮询）', () => {
    expect(planPollDelayMs(1_000, 5_000, rand0)).toBe(0)
  })

  it('任意随机值下延迟落在 [max(0, interval-jitter), interval+jitter] 内', () => {
    for (let i = 0; i < 100; i++) {
      const delay = planPollDelayMs(INTERVAL, JITTER, Math.random)
      expect(delay).toBeGreaterThanOrEqual(INTERVAL - JITTER)
      expect(delay).toBeLessThanOrEqual(INTERVAL + JITTER)
    }
  })
})

describe('createPoller — 基本节奏', () => {
  it('start() 不立即执行 task，首次执行在一个完整延迟之后', () => {
    const clock = new FakeClock()
    let calls = 0
    const poller = makePoller(clock, { task: () => { calls++ } })
    poller.start()
    expect(calls).toBe(0)
    clock.advance(INTERVAL - 1)
    expect(calls).toBe(0)
    clock.advance(1)
    expect(calls).toBe(1)
  })

  it('递归排程：每周期执行一次，任意时刻只有一个待触发句柄', () => {
    const clock = new FakeClock()
    let calls = 0
    const poller = makePoller(clock, { task: () => { calls++ } })
    poller.start()
    clock.advance(INTERVAL * 3)
    expect(calls).toBe(3)
    expect(clock.pending).toBe(1)
  })

  it('抖动逐周期生效（固定间隔的 setInterval 做不到）', () => {
    const clock = new FakeClock()
    const rands = [rand0, rand1, randHalf]
    let i = 0
    const firedAt: number[] = []
    const poller = makePoller(clock, {
      task: () => { firedAt.push(clock.nowMs) },
      jitterMs: JITTER,
      rand: () => rands[i++ % rands.length](),
    })
    poller.start()
    clock.advance(INTERVAL * 3 + JITTER)
    // 三个周期的延迟依次为 -2s / +2s / ±0
    expect(firedAt).toEqual([INTERVAL - JITTER, 2 * INTERVAL, 3 * INTERVAL])
  })
})

describe('createPoller — start/stop 幂等', () => {
  it('start() 幂等：重复调用不产生多个定时器', () => {
    const clock = new FakeClock()
    let calls = 0
    const poller = makePoller(clock, { task: () => { calls++ } })
    poller.start()
    poller.start()
    poller.start()
    expect(clock.pending).toBe(1)
    clock.advance(INTERVAL)
    expect(calls).toBe(1)
  })

  it('stop() 清理定时器句柄且幂等，之后不再执行 task', () => {
    const clock = new FakeClock()
    let calls = 0
    const poller = makePoller(clock, { task: () => { calls++ } })
    poller.start()
    poller.stop()
    poller.stop()
    expect(clock.pending).toBe(0)
    clock.advance(INTERVAL * 3)
    expect(calls).toBe(0)
  })

  it('isRunning 反映生命周期；stop 后可重新 start', () => {
    const clock = new FakeClock()
    let calls = 0
    const poller = makePoller(clock, { task: () => { calls++ } })
    expect(poller.isRunning()).toBe(false)
    poller.start()
    expect(poller.isRunning()).toBe(true)
    poller.stop()
    expect(poller.isRunning()).toBe(false)
    poller.start()
    expect(poller.isRunning()).toBe(true)
    clock.advance(INTERVAL)
    expect(calls).toBe(1)
  })
})

describe('createPoller — 暂停与容错', () => {
  it('isPaused 为 true 时跳过本次 task，但仍排下一周期', () => {
    const clock = new FakeClock()
    let calls = 0
    let paused = true
    const poller = makePoller(clock, {
      task: () => { calls++ },
      isPaused: () => paused,
    })
    poller.start()
    clock.advance(INTERVAL)
    expect(calls).toBe(0)
    // 暂停期间轮询节奏不中断：下一周期已排入
    expect(clock.pending).toBe(1)
    paused = false
    clock.advance(INTERVAL)
    expect(calls).toBe(1)
  })

  it('task 同步抛错不终止轮询，错误交给 onError', () => {
    const clock = new FakeClock()
    const errors: unknown[] = []
    let calls = 0
    const poller = makePoller(clock, {
      task: () => {
        calls++
        throw new Error('boom')
      },
      onError: (e) => errors.push(e),
    })
    poller.start()
    clock.advance(INTERVAL * 3)
    expect(calls).toBe(3)
    expect(errors).toHaveLength(3)
    expect(poller.isRunning()).toBe(true)
  })

  it('task 返回拒绝的 Promise 同样不终止轮询', async () => {
    const clock = new FakeClock()
    const onError = vi.fn()
    let calls = 0
    const poller = makePoller(clock, {
      task: async () => {
        calls++
        throw new Error('async boom')
      },
      onError,
    })
    poller.start()
    clock.advance(INTERVAL)
    await flushAsync()
    expect(onError).toHaveBeenCalledTimes(1)
    clock.advance(INTERVAL)
    await flushAsync()
    expect(calls).toBe(2)
    expect(poller.isRunning()).toBe(true)
  })
})

describe('createPoller — 重入保护', () => {
  it('上一次 task 未 resolve 时到点不并发执行，只续排下一周期', async () => {
    const clock = new FakeClock()
    let started = 0
    let resolveFirst: (() => void) | null = null
    const poller = makePoller(clock, {
      task: () =>
        new Promise<void>((resolve) => {
          started++
          if (started === 1) resolveFirst = resolve
          else resolve()
        }),
    })
    poller.start()
    clock.advance(INTERVAL)
    expect(started).toBe(1)
    // 首个 task 悬挂期间连过三个周期：不叠加并发请求
    clock.advance(INTERVAL * 3)
    await flushAsync()
    expect(started).toBe(1)
    expect(clock.pending).toBe(1)
    // 悬挂的 task 完成后，下一周期恢复正常执行
    resolveFirst!()
    await flushAsync()
    clock.advance(INTERVAL)
    expect(started).toBe(2)
  })

  it('慢 task 完成后 busy 标志复位，不会永久跳过', async () => {
    const clock = new FakeClock()
    let calls = 0
    const poller = makePoller(clock, {
      task: async () => {
        calls++
        await flushAsync()
      },
    })
    poller.start()
    clock.advance(INTERVAL)
    await flushAsync()
    clock.advance(INTERVAL)
    await flushAsync()
    expect(calls).toBe(2)
  })
})
