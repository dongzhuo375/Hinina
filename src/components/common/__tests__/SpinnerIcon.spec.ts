// @vitest-environment jsdom
import { createApp, h, nextTick } from 'vue'
import { describe, expect, it } from 'vitest'
import SpinnerIcon from '@/components/common/SpinnerIcon.vue'

function mount(props: { strokeWidth?: number | string; class?: string } = {}) {
  const host = document.createElement('div')
  const app = createApp({ render: () => h(SpinnerIcon, props) })
  app.mount(host)
  return host
}

describe('SpinnerIcon', () => {
  it('旋转图标：animate-spin + 25% 底圈 + 90° 弧线，默认线宽 2', async () => {
    const host = mount()
    await nextTick()
    const svg = host.querySelector('svg')!
    expect(svg.className.baseVal).toContain('animate-spin')
    expect(svg.getAttribute('stroke-width')).toBe('2')
    expect(svg.querySelector('circle')?.getAttribute('stroke-opacity')).toBe('0.25')
    expect(svg.querySelector('path')?.getAttribute('d')).toBe('M21 12a9 9 0 00-9-9')
  })

  it('class 透传合并（尺寸/颜色）且 strokeWidth 可覆盖（状态 pill 用 2.5）', async () => {
    const host = mount({ class: 'h-3 w-3 text-[var(--color-warning)]', strokeWidth: 2.5 })
    await nextTick()
    const svg = host.querySelector('svg')!
    expect(svg.className.baseVal).toContain('h-3 w-3')
    expect(svg.className.baseVal).toContain('animate-spin')
    expect(svg.getAttribute('stroke-width')).toBe('2.5')
  })
})
