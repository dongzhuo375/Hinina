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

  function releaseSpyLock(): void {
    spyLockTarget = null
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

  function scrollToSection(index: number): void {
    activeSection.value = index
    spyLockTarget = index
    if (spyLockTimer) clearTimeout(spyLockTimer)
    spyLockTimer = setTimeout(releaseSpyLock, 1_000)
    const target = document.getElementById(`settings-${index}`)
    target?.scrollIntoView({ block: 'start', behavior: 'smooth' })
    target?.focus({ preventScroll: true })
  }

  // sections 仅在配置加载成功后渲染，渲染完成后再做一次高亮校正
  watch(loading, (isLoading) => {
    if (!isLoading) void nextTick(updateActiveSection)
  })

  onScopeDispose(releaseSpyLock)

  return { activeSection, scrollerRef, updateActiveSection, scrollToSection, SECTION_LABELS }
}
