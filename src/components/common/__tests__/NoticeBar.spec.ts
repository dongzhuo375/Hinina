// @vitest-environment jsdom
import { createApp, h, nextTick } from 'vue'
import { describe, expect, it, vi } from 'vitest'
import NoticeBar from '@/components/common/NoticeBar.vue'

function mount(props: { tone: 'wa' | 'pending'; retry?: () => void }, slotText = '刷新失败：超时') {
  const host = document.createElement('div')
  const app = createApp({
    render: () => h(NoticeBar, props, { default: () => slotText }),
  })
  app.mount(host)
  return host
}

describe('NoticeBar', () => {
  it('wa tone：容器配色 + 默认三角警示图标 + 消息插槽', async () => {
    const host = mount({ tone: 'wa' })
    await nextTick()
    const bar = host.firstElementChild as HTMLElement
    expect(bar.className).toContain('border-[var(--color-wa-border)]')
    expect(bar.className).toContain('bg-[var(--color-wa-bg)]')
    expect(bar.className).toContain('text-[var(--color-error)]')
    expect(bar.querySelector('svg')).not.toBeNull()
    expect(bar.textContent).toContain('刷新失败：超时')
  })

  it('pending tone：pending 配色且不渲染默认三角图标（调用方经 #icon 提供或无图标）', async () => {
    const host = mount({ tone: 'pending' }, '榜单已封榜')
    await nextTick()
    const bar = host.firstElementChild as HTMLElement
    expect(bar.className).toContain('border-[var(--color-pending-border)]')
    expect(bar.className).toContain('text-[var(--color-warning)]')
    expect(bar.querySelector('svg')).toBeNull()
  })

  it('retry 透传：渲染立即重试按钮并转发点击；未传时不渲染按钮', async () => {
    const retry = vi.fn()
    const host = mount({ tone: 'wa', retry })
    await nextTick()
    const button = host.querySelector<HTMLButtonElement>('button')!
    expect(button.textContent).toContain('立即重试')
    button.click()
    expect(retry).toHaveBeenCalledTimes(1)

    const host2 = mount({ tone: 'wa' })
    await nextTick()
    expect(host2.querySelector('button')).toBeNull()
  })
})
