import { afterEach, describe, expect, it, vi } from 'vitest'
import { createCloseGuard } from '@/utils/close-guard'

/// 可控的落盘 Promise：测试需要「落盘永不 settle」这类极端情形
function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

interface Overrides {
  flush?: () => Promise<unknown>
  close?: () => Promise<unknown>
  destroy?: () => Promise<unknown>
  flushTimeoutMs?: number
  hardTimeoutMs?: number
}

interface Harness {
  guard: ReturnType<typeof createCloseGuard>
  flush: ReturnType<typeof vi.fn>
  close: ReturnType<typeof vi.fn>
  destroy: ReturnType<typeof vi.fn>
  warn: ReturnType<typeof vi.fn>
}

/**
 * 构造守卫与配套 spy。
 *
 * 覆写的依赖会被**包成 spy 后再注入**，因此断言用的 spy 与守卫实际调用的
 * 是同一个函数（早期版本直接把 overrides 展开进 createCloseGuard，导致断言的
 * 是未被使用的默认 spy，测试永远看到 0 次调用）。
 */
function makeGuard(overrides: Overrides = {}): Harness {
  const flush = vi.fn(overrides.flush ?? (async () => undefined))
  const close = vi.fn(overrides.close ?? (async () => undefined))
  const destroy = vi.fn(overrides.destroy ?? (async () => undefined))
  const warn = vi.fn()
  const guard = createCloseGuard({
    flush,
    close,
    destroy,
    onWarn: warn,
    flushTimeoutMs: overrides.flushTimeoutMs,
    hardTimeoutMs: overrides.hardTimeoutMs,
  })
  return { guard, flush, close, destroy, warn }
}

/**
 * 冲刷微任务与宏任务队列（守卫的收尾跨多个 tick：race → finalize → destroy）。
 *
 * 用真实定时器（`setTimeout 0`）而不是 `await Promise.resolve()`：后者的 tick 数
 * 取决于 promise 链长度，写死次数会在链路稍变时变得脆弱。
 * 因此**只有需要推进时间的用例才开 fake timers**，其余一律真实定时器。
 */
const settle = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0))

afterEach(() => {
  vi.useRealTimers()
})

describe('关窗守卫 — 正常路径', () => {
  it('首次请求拦截，落盘完成后销毁窗口', async () => {
    const { guard, flush, destroy } = makeGuard()

    expect(guard.handleRequest()).toBe(true)
    expect(guard.state()).toBe('flushing')

    await settle()

    expect(flush).toHaveBeenCalledTimes(1)
    expect(destroy).toHaveBeenCalledTimes(1)
    expect(guard.state()).toBe('allowing')
  })

  it('进入 allowing 后再次请求直接放行', async () => {
    const { guard } = makeGuard()

    guard.handleRequest()
    await settle()

    expect(guard.handleRequest()).toBe(false)
  })

  it('落盘在途时重复请求仍拦截且不重复落盘', async () => {
    // 不耐烦的双击不该中断在途落盘（等于零超时丢数据）
    const pending = deferred<void>()
    const { guard, flush, destroy } = makeGuard({ flush: () => pending.promise })

    expect(guard.handleRequest()).toBe(true)
    expect(guard.handleRequest()).toBe(true)
    expect(guard.handleRequest()).toBe(true)
    expect(flush).toHaveBeenCalledTimes(1)
    expect(destroy).not.toHaveBeenCalled()

    pending.resolve()
    await settle()

    expect(destroy).toHaveBeenCalledTimes(1)
    expect(guard.handleRequest()).toBe(false)
  })
})

describe('关窗守卫 — 失效路径（原实现的 bug 现场）', () => {
  it('落盘抛错也必须放行关闭', async () => {
    const { guard, destroy } = makeGuard({ flush: async () => { throw new Error('IPC 断线') } })

    guard.handleRequest()
    await settle()

    expect(destroy).toHaveBeenCalledTimes(1)
    expect(guard.handleRequest()).toBe(false)
  })

  it('destroy 失败时退回常规 close（窗口仍能关上）', async () => {
    const { guard, close, destroy, warn } = makeGuard({ destroy: async () => { throw new Error('permission denied') } })

    guard.handleRequest()
    await settle()

    expect(destroy).toHaveBeenCalledTimes(1)
    expect(close).toHaveBeenCalledTimes(1)
    expect(warn).toHaveBeenCalled()
    expect(guard.handleRequest()).toBe(false)
  })

  it('destroy 与 close 都失败后不再永久卡死（状态已是 allowing）', async () => {
    // 原实现正是在这里卡死：flushing 永久为 true，之后每次点关闭都被 preventDefault 吞掉
    const { guard, warn } = makeGuard({ destroy: async () => { throw new Error('destroy failed') }, close: async () => { throw new Error('close failed') } })

    guard.handleRequest()
    await settle()

    expect(warn).toHaveBeenCalledTimes(2)
    expect(guard.state()).toBe('allowing')
    expect(guard.handleRequest()).toBe(false)
  })
})

describe('关窗守卫 — 时间上界', () => {
  it('落盘永不 settle 时由 flushTimeoutMs 兜底关闭', async () => {
    // 原实现的致命点：超时只让 Promise.race 返回，收尾却依赖一次可能失败的 close()
    vi.useFakeTimers()
    const never = deferred<void>()
    const { guard, destroy } = makeGuard({ flush: () => never.promise })

    guard.handleRequest()
    expect(destroy).not.toHaveBeenCalled()

    await vi.advanceTimersByTimeAsync(3_000)

    expect(destroy).toHaveBeenCalledTimes(1)
    expect(guard.handleRequest()).toBe(false)
  })

  it('硬超时是绝对上界：落盘上界被配得更大时仍由硬超时兜住', async () => {
    // 硬超时的存在意义是「从拦截那一刻起的绝对上界」，不依赖落盘超时的配置正确性
    vi.useFakeTimers()
    const never = deferred<void>()
    const { guard, destroy } = makeGuard({ flush: () => never.promise, flushTimeoutMs: 10_000, hardTimeoutMs: 1_000 })

    guard.handleRequest()
    await vi.advanceTimersByTimeAsync(1_000)

    expect(destroy).toHaveBeenCalledTimes(1)
    expect(guard.state()).toBe('allowing')
  })

  it('硬超时不会让落盘成功的正常路径重复销毁窗口', async () => {
    const { guard, destroy } = makeGuard()

    guard.handleRequest()
    await settle()
    // 再等一小段真实时间：finalize 幂等，不会因定时器到点重复销毁
    await new Promise((resolve) => setTimeout(resolve, 20))

    expect(destroy).toHaveBeenCalledTimes(1)
  })

  it('flushTimeoutMs 控制放行时刻', async () => {
    vi.useFakeTimers()
    const never = deferred<void>()
    const { guard, destroy } = makeGuard({ flush: () => never.promise, flushTimeoutMs: 500, hardTimeoutMs: 2_000 })

    guard.handleRequest()
    await vi.advanceTimersByTimeAsync(499)
    expect(destroy).not.toHaveBeenCalled()

    await vi.advanceTimersByTimeAsync(1)
    expect(destroy).toHaveBeenCalledTimes(1)
  })
})
