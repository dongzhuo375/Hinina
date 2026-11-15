// @vitest-environment jsdom
import { createApp, nextTick } from 'vue'
import { describe, expect, it } from 'vitest'
import SettingsField from '@/components/settings/SettingsField.vue'

function mount(template: string): HTMLElement {
  const host = document.createElement('div')
  const app = createApp({
    components: { SettingsField },
    template,
  })
  app.mount(host)
  return host
}

describe('SettingsField', () => {
  it('label 首个直接子 span 携带标签文本、控件渲染在 label 内（field() 定位契约）', async () => {
    const host = mount(`
      <SettingsField label="请求超时（秒）" hint="1–120">
        <input type="text" inputmode="numeric" />
      </SettingsField>
    `)
    await nextTick()
    const label = host.querySelector('label')!
    expect(label).toBeTruthy()
    // SettingsView.behavior.spec 的 field() 按 :scope > span 的文本找字段
    expect(label.querySelector(':scope > span')?.textContent).toContain('请求超时（秒）')
    expect(label.querySelector('input, select')).toBeTruthy()
    // 常态：提示行可见、无错误行
    expect(label.textContent).toContain('1–120')
  })

  it('error 存在时替代提示行；locked 降为 muted 色', async () => {
    const host = mount(`
      <SettingsField label="字号" hint="8–32" error="超出范围" locked>
        <input type="text" />
      </SettingsField>
    `)
    await nextTick()
    const label = host.querySelector('label')!
    expect(label.textContent).toContain('超出范围')
    expect(label.textContent).not.toContain('8–32')
    const labelSpan = label.querySelector(':scope > span') as HTMLElement
    expect(labelSpan.className).toContain('text-[var(--text-muted)]')
  })

  it('未传 error/hint 时不渲染尾部行（ThemeSection 的只读字段形态）', async () => {
    const host = mount(`
      <SettingsField label="界面主题">
        <select disabled><option>浅色</option></select>
      </SettingsField>
    `)
    await nextTick()
    const label = host.querySelector('label')!
    expect(label.querySelectorAll('span')).toHaveLength(1)
  })
})
