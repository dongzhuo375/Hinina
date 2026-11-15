// @vitest-environment jsdom
import { createApp, nextTick } from 'vue'
import { describe, expect, it, vi } from 'vitest'
import AboutInfoRow from '@/components/settings/AboutInfoRow.vue'

function mount(props: Partial<InstanceType<typeof AboutInfoRow>['$props']> = {}) {
  const host = document.createElement('div')
  const app = createApp(AboutInfoRow, {
    label: '客户端版本',
    value: '0.1.2',
    copyKey: 'version',
    copiedKey: null,
    copyText: vi.fn(async () => {}),
    ...props,
  })
  app.mount(host)
  return host
}

describe('AboutInfoRow', () => {
  it('渲染 dt 标签 / 等宽截断值 / 复制按钮，点击以 (copyKey, value) 调用 copyText', async () => {
    const copyText = vi.fn(async () => {})
    const host = mount({ copyText })
    await nextTick()

    expect(host.querySelector('dt')?.textContent).toContain('客户端版本')
    const dd = host.querySelector('dd')
    expect(dd?.textContent).toContain('0.1.2')
    expect(dd?.className).toContain('truncate font-mono')

    host.querySelector('button')!.click()
    await nextTick()
    expect(copyText).toHaveBeenCalledWith('version', '0.1.2')
  })

  it('title 透传为原生 title 属性；copiedKey 命中时按钮文案切换为已复制', async () => {
    const host = mount({ title: 'D:\\data', copiedKey: 'baseDir', copyKey: 'baseDir' })
    await nextTick()
    expect(host.querySelector('dd')?.getAttribute('title')).toBe('D:\\data')
    expect(host.querySelector('button')?.textContent).toContain('已复制')
  })

  it('未命中 copiedKey 或未传 title 时：复制文案为复制、无 title 属性', async () => {
    const host = mount({ copiedKey: 'baseDir', copyKey: 'version' })
    await nextTick()
    expect(host.querySelector('button')?.textContent).toContain('复制')
    expect(host.querySelector('button')?.textContent).not.toContain('已复制')
    expect(host.querySelector('dd')?.hasAttribute('title')).toBe(false)
  })
})
