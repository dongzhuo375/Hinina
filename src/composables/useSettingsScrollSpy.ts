import { nextTick, onScopeDispose, ref, watch } from 'vue'
import type { Ref } from 'vue'

export const SECTION_LABELS: readonly string[] = [
  'OJ 服务器',
  '编辑器',
  '布局',
  '主题',
  '重置与清理',
  '数据目录',
  '关于',
]

const SPY_OFFSET_PX = 96

/// 设置页左侧索引的 scroll-spy 状态机；每个组件实例独立持有锁和 timer
export function useSettingsScrollSpy(loading: Readonly<Ref<boolean>>) {
  const activeSection = ref(0)
  const scrollerRef = ref<HTMLElement | null>(null)
  let spyLockTarget: number | null = null
  let spyLockTimer: ReturnType<typeof setTimeout> | null = null
  let scrollEndScroller: HTMLElement | null = null
  let scrollEndHandler: (() => void) | null = null
  let scheduledFrame: ReturnType<typeof setTimeout> | null = null

  function detachScrollEnd(): void {
    if (scrollEndScroller && scrollEndHandler) {
      scrollEndScroller.removeEventListener('scrollend', scrollEndHandler)
    }
    scrollEndScroller = null
    scrollEndHandler = null
  }

  function releaseSpyLock(): void {
    spyLockTarget = null
    detachScrollEnd()
    if (spyLockTimer) {
      clearTimeout(spyLockTimer)
      spyLockTimer = null
    }
  }

  function updateActiveSection(): void {
    const scroller = scrollerRef.value
    if (!scroller) return
    const scrollerTop = scroller.getBoundingClientRect().top
    // 只有页面真的可滚动时才采用 atBottom，避免无需滚动时首屏误判最后一组
    const scrollable = scroller.scrollHeight > scroller.clientHeight + 2
    const atBottom =
      scrollable && scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 2

    if (spyLockTarget !== null) {
      const target = document.getElementById(`settings-${spyLockTarget}`)
      const docked =
        target !== null && Math.abs(target.getBoundingClientRect().top - scrollerTop - 24) <= 2
      // atBottom 只释放本来就是最后一项的锁；从底部回点上方组时不得提前解锁
      const settled = docked || (atBottom && spyLockTarget === SECTION_LABELS.length - 1)
      if (!settled) return
      releaseSpyLock()
    }

    if (atBottom) {
      activeSection.value = SECTION_LABELS.length - 1
      return
    }

    const anchorTop = scrollerTop + SPY_OFFSET_PX
    let current = 0
    for (let i = 0; i < SECTION_LABELS.length; i++) {
      const el = document.getElementById(`settings-${i}`)
      if (el && el.getBoundingClientRect().top <= anchorTop) current = i
    }
    activeSection.value = current
  }

  /// 滚动事件只调度一次下一帧扫描，避免同帧多个事件重复读取 7 个分区布局
  function scheduleActiveSection(): void {
    if (scheduledFrame !== null) return
    scheduledFrame = setTimeout(() => {
      scheduledFrame = null
      updateActiveSection()
    }, 0)
  }

  function scrollToSection(index: number): void {
    releaseSpyLock()
    activeSection.value = index
    spyLockTarget = index
    // 平滑滚动可能超过 1s；锁必须持续到 scrollend 或目标停靠，不能按固定短超时提前释放
    spyLockTimer = setTimeout(releaseSpyLock, 3_000)
    const scroller = scrollerRef.value
    if (scroller) {
      scrollEndScroller = scroller
      scrollEndHandler = () => {
        releaseSpyLock()
        updateActiveSection()
      }
      scroller.addEventListener('scrollend', scrollEndHandler, { once: true })
    }
    const target = document.getElementById(`settings-${index}`)
    target?.scrollIntoView({ block: 'start', behavior: 'smooth' })
    target?.focus({ preventScroll: true })
  }

  // sections 仅在配置加载成功后渲染，渲染完成后再做一次高亮校正
  watch(loading, (isLoading) => {
    if (!isLoading) void nextTick(updateActiveSection)
  })

  onScopeDispose(() => {
    if (scheduledFrame !== null) clearTimeout(scheduledFrame)
    scheduledFrame = null
    releaseSpyLock()
  })

  return {
    activeSection,
    scrollerRef,
    updateActiveSection,
    scheduleActiveSection,
    scrollToSection,
    SECTION_LABELS,
  }
}
