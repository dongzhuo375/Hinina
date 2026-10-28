import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { effectScope } from 'vue'
import type { EffectScope } from 'vue'
import { useTransientValue } from '@/composables/useTransientValue'

let scope: EffectScope

beforeEach(() => {
  vi.useFakeTimers()
  scope = effectScope()
})

afterEach(() => {
  scope.stop()
  vi.useRealTimers()
})

describe('useTransientValue', () => {
  it('show 持有临时值，到期恢复初值', () => {
    const feedback = scope.run(() => useTransientValue(false, 1_500))!
    expect(feedback.value.value).toBe(false)

    feedback.show(true)
    expect(feedback.value.value).toBe(true)
    vi.advanceTimersByTime(1_499)
    expect(feedback.value.value).toBe(true)
    vi.advanceTimersByTime(1)
    expect(feedback.value.value).toBe(false)
  })

  it('连续 show 取消旧 timer，不让旧值提前清掉新反馈', () => {
    const feedback = scope.run(() => useTransientValue(-1, 1_500))!
    feedback.show(1)
    vi.advanceTimersByTime(1_000)
    feedback.show(2)
    vi.advanceTimersByTime(1_000)
    expect(feedback.value.value).toBe(2)
    vi.advanceTimersByTime(500)
    expect(feedback.value.value).toBe(-1)
  })

  it('clear 立即复位并取消待执行回弹', () => {
    const feedback = scope.run(() => useTransientValue<string | null>(null, 2_000))!
    feedback.show('version')
    feedback.clear()
    expect(feedback.value.value).toBeNull()
    vi.advanceTimersByTime(2_000)
    expect(feedback.value.value).toBeNull()
  })

  it('不同实例的状态和时长互不影响', () => {
    const first = scope.run(() => useTransientValue<string | null>(null, 1_000))!
    const second = scope.run(() => useTransientValue<string | null>(null, 2_000))!
    first.show('a')
    second.show('b')
    vi.advanceTimersByTime(1_000)
    expect(first.value.value).toBeNull()
    expect(second.value.value).toBe('b')
    vi.advanceTimersByTime(1_000)
    expect(second.value.value).toBeNull()
  })

  it('scope dispose 清 timer；销毁后 show/clear 不再写状态或创建 timer', () => {
    const feedback = scope.run(() => useTransientValue(false, 1_500))!
    feedback.show(true)
    scope.stop()
    expect(vi.getTimerCount()).toBe(0)

    feedback.show(true)
    feedback.clear()
    expect(vi.getTimerCount()).toBe(0)
    expect(feedback.value.value).toBe(true)
  })
})
