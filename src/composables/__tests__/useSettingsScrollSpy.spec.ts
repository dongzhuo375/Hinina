// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { effectScope, nextTick, ref } from 'vue'
import type { EffectScope, Ref } from 'vue'
import { useSettingsScrollSpy } from '@/composables/useSettingsScrollSpy'

let scope: EffectScope
let loading: Ref<boolean>
let spy: ReturnType<typeof useSettingsScrollSpy>
let scroller: HTMLElement
let sections: HTMLElement[]
let tops: number[]

beforeEach(() => {
  vi.useFakeTimers()
  document.body.innerHTML = ''
  scroller = document.createElement('div')
  document.body.append(scroller)
  scroller.getBoundingClientRect = () => ({ top: 0 } as DOMRect)
  sections = Array.from({ length: 7 }, (_, index) => {
    const element = document.createElement('section')
    element.id = `settings-${index}`
    element.getBoundingClientRect = () => ({ top: tops[index] } as DOMRect)
    element.scrollIntoView = vi.fn()
    element.focus = vi.fn()
    scroller.append(element)
    return element
  })
  tops = [0, 200, 400, 600, 800, 1_000, 1_200]
  Object.defineProperty(scroller, 'scrollHeight', { configurable: true, value: 2_000, writable: true })
  Object.defineProperty(scroller, 'clientHeight', { configurable: true, value: 500, writable: true })
  scope = effectScope()
  loading = ref(true)
  spy = scope.run(() => useSettingsScrollSpy(loading))!
  spy.scrollerRef.value = scroller
})

afterEach(() => {
  scope.stop()
  document.body.innerHTML = ''
  vi.useRealTimers()
  vi.restoreAllMocks()
})

describe('useSettingsScrollSpy', () => {
  it('不可滚动时不误判末组；普通滚动取判定线前最后一组，atBottom 取末组', () => {
    Object.defineProperty(scroller, 'scrollHeight', { configurable: true, value: 500 })
    spy.updateActiveSection()
    expect(spy.activeSection.value).toBe(0)

    Object.defineProperty(scroller, 'scrollHeight', { configurable: true, value: 2_000 })
    tops = [0, 20, 200, 400, 600, 800, 1_000]
    spy.updateActiveSection()
    expect(spy.activeSection.value).toBe(1)

    scroller.scrollTop = 1_500
    spy.updateActiveSection()
    expect(spy.activeSection.value).toBe(6)
  })

  it('非最后目标在 atBottom 时不得提前解锁，1 秒超时后才交还滚动驱动', () => {
    spy.scrollToSection(2)
    expect(spy.activeSection.value).toBe(2)
    expect(sections[2].scrollIntoView).toHaveBeenCalledWith({ block: 'start', behavior: 'smooth' })
    expect(sections[2].focus).toHaveBeenCalledWith({ preventScroll: true })

    scroller.scrollTop = 1_500
    spy.updateActiveSection()
    expect(spy.activeSection.value).toBe(2)

    vi.advanceTimersByTime(1_000)
    spy.updateActiveSection()
    expect(spy.activeSection.value).toBe(2)

    scroller.dispatchEvent(new Event('scrollend'))
    expect(spy.activeSection.value).toBe(6)
    expect(vi.getTimerCount()).toBe(0)
  })

  it('目标到达 24px 停靠点立即释放锁并清理 timer', () => {
    spy.scrollToSection(1)
    tops = [0, 24, 400, 600, 800, 1_000, 1_200]
    spy.updateActiveSection()

    expect(spy.activeSection.value).toBe(1)
    expect(vi.getTimerCount()).toBe(0)
  })

  it('最后一组可用 atBottom 释放；scope dispose 清理锁和 timer', () => {
    spy.scrollToSection(6)
    scroller.scrollTop = 1_500
    spy.updateActiveSection()
    expect(vi.getTimerCount()).toBe(0)

    spy.scrollToSection(3)
    expect(vi.getTimerCount()).toBe(1)
    scope.stop()
    expect(vi.getTimerCount()).toBe(0)
  })

  it('loading 结束后 nextTick 校正高亮', async () => {
    tops = [0, 20, 200, 400, 600, 800, 1_000]
    loading.value = false
    await nextTick()
    await nextTick()
    expect(spy.activeSection.value).toBe(1)
  })
})
