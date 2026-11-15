// @vitest-environment jsdom
import { createApp, h, nextTick } from 'vue'
import { describe, expect, it } from 'vitest'
import SettingsSwitch from '@/components/settings/SettingsSwitch.vue'

function mount(checked: boolean, disabled?: boolean): HTMLElement {
  const host = document.createElement('div')
  const app = createApp(SettingsSwitch, { label: '题面缓存', checked, disabled })
  app.mount(host)
  return host
}

describe('SettingsSwitch', () => {
  it('role=switch 按钮 + 圆钮 class + 已开启/已关闭状态文本', async () => {
    const host = mount(true)
    await nextTick()
    const button = host.querySelector<HTMLButtonElement>('button[role="switch"]')!
    // 按钮内无可见文本，可访问名经 aria-label 提供
    expect(button.getAttribute('aria-label')).toBe('题面缓存')
    expect(button.getAttribute('aria-checked')).toBe('true')
    // settings-switch(-knob) class 由 SettingsSectionFrame 的 :deep 样式消费
    expect(button.className).toContain('settings-switch')
    expect(button.querySelector('span')?.className).toContain('settings-switch-knob')
    expect(host.textContent).toContain('已开启')

    button.click()
    await nextTick()
    // 无受控回写时保持传入值（checked 由调用方经 update 事件回写）
    expect(button.getAttribute('aria-checked')).toBe('true')
  })

  it('点击触发 update 事件携带反转值；关闭态文本', async () => {
    const received: boolean[] = []
    const host = document.createElement('div')
    const app = createApp({
      render: () =>
        h(SettingsSwitch, {
          label: '自动保存',
          checked: false,
          onUpdate: (checked: boolean) => received.push(checked),
        }),
    })
    app.mount(host)
    await nextTick()
    expect(host.textContent).toContain('已关闭')
    host.querySelector('button')!.click()
    await nextTick()
    expect(received).toEqual([true])
  })

  it('disabled 时按钮禁用', async () => {
    const host = mount(true, true)
    await nextTick()
    expect((host.querySelector('button') as HTMLButtonElement).disabled).toBe(true)
    expect(host.querySelector('button')!.className).toContain('disabled:opacity-60')
  })
})
