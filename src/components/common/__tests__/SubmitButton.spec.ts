// @vitest-environment jsdom
import { createApp, h, nextTick } from 'vue'
import { describe, expect, it } from 'vitest'
import SubmitButton from '@/components/common/SubmitButton.vue'

function mount(props: { busy: boolean; pending: boolean; tone: 'success' | 'primary'; tracking?: boolean }) {
  const host = document.createElement('div')
  const app = createApp({ render: () => h(SubmitButton, props) })
  app.mount(host)
  return host
}

describe('SubmitButton', () => {
  it('常态：提交代码 + 纸飞机图标、可点击、tone 决定底色', async () => {
    const host = mount({ tone: 'success', busy: false, pending: false })
    await nextTick()
    const button = host.querySelector<HTMLButtonElement>('button')!
    expect(button.textContent).toContain('提交代码')
    expect(button.className).toContain('bg-[var(--color-success)]')
    expect(button.querySelector('svg')).not.toBeNull()
    expect(button.disabled).toBe(false)
    expect(button.getAttribute('aria-busy')).toBe('false')

    const primary = mount({ tone: 'primary', busy: false, pending: false, tracking: true })
    await nextTick()
    const pb = primary.querySelector('button')!
    expect(pb.className).toContain('bg-[var(--color-primary)]')
    expect(pb.querySelector('span')!.className).toContain('tracking-wide')
  })

  it('busy：提交中… + spinner + 禁用 + aria-busy', async () => {
    const host = mount({ tone: 'success', busy: true, pending: false })
    await nextTick()
    const button = host.querySelector<HTMLButtonElement>('button')!
    expect(button.textContent).toContain('提交中…')
    expect(button.querySelector('svg.animate-spin')).not.toBeNull()
    expect(button.disabled).toBe(true)
    expect(button.getAttribute('aria-busy')).toBe('true')
  })

  it('pending：评测中… + 禁用（防重复提交遮蔽）', async () => {
    const host = mount({ tone: 'primary', busy: false, pending: true })
    await nextTick()
    const button = host.querySelector<HTMLButtonElement>('button')!
    expect(button.textContent).toContain('评测中…')
    expect(button.textContent).not.toContain('提交代码')
    expect(button.disabled).toBe(true)
  })
})
