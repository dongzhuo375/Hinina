// @vitest-environment jsdom
import { createApp, nextTick, ref } from 'vue'
import { describe, expect, it } from 'vitest'
import AboutSection from '@/components/settings/AboutSection.vue'
import { useAboutSettings } from '@/composables/useAboutSettings'

/// 组件只消费模型形状：以同形 refs 直建模型，不经 pinia/IPC
const makeModel = (storage: string | null, failed = false) => ({
  storage: ref(storage === null ? null : { version: '0.3.5', baseDir: 'D:\\data', logPath: 'D:\\logs\\a.log' }),
  storageFailed: ref(failed),
  loadStorage: async () => {},
  copiedKey: ref<string | null>(null),
  copyText: async () => {},
})

describe('AboutSection 项目仓库行', () => {
  it('渲染仓库外链（GitHub 内联 SVG + slug），复用拦截器转交故无 target', async () => {
    const host = document.createElement('div')
    const app = createApp(AboutSection, { model: makeModel('0.3.5') })
    app.mount(host)
    await nextTick()

    const link = host.querySelector<HTMLAnchorElement>(
      'a[href="https://github.com/dongzhuo375/Hinina"]',
    )
    // 无 aria-label：可见 slug 天然成可访问名（WCAG 2.5.3 label-in-name，
    // 语音输入说 slug 即匹配）；重新加回覆盖型 aria-label 会让下方断言变红
    expect(link?.textContent).toContain('dongzhuo375/Hinina')
    expect(link?.querySelector('svg')).not.toBeNull()
    expect(link?.hasAttribute('aria-label')).toBe(false)
    expect(link?.hasAttribute('target')).toBe(false)
    app.unmount()
  })

  it('存储读取中/失败态下仓库行仍然可见', async () => {
    for (const failed of [false, true]) {
      const host = document.createElement('div')
      const app = createApp(AboutSection, { model: makeModel(null, failed) })
      app.mount(host)
      await nextTick()
      expect(host.querySelector('a[href^="https://github.com/dongzhuo375"]')).not.toBeNull()
      app.unmount()
    }
  })
})
