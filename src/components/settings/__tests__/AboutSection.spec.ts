// @vitest-environment jsdom
import { createApp, nextTick, ref } from 'vue'
import { describe, expect, it } from 'vitest'
import AboutSection from '@/components/settings/AboutSection.vue'
import { useAboutSettings } from '@/composables/useAboutSettings'

/// 组件只消费模型形状：返回值锚定 composable 返回类型，fixture 漂移编译期即红
const makeModel = (
  version: string | null,
  failed = false,
): ReturnType<typeof useAboutSettings> => ({
  storage: ref(
    version === null
      ? null
      : { version, baseDir: 'D:\\data', logPath: 'D:\\logs\\a.log' },
  ),
  storageFailed: ref(failed),
  loadStorage: async () => {},
  copiedKey: ref<string | null>(null),
  copyText: async () => {},
})

describe('AboutSection 项目仓库行', () => {
  it('渲染仓库外链（GitHub 内联 SVG + slug）并带 target fail-safe', async () => {
    const host = document.createElement('div')
    const app = createApp(AboutSection, { model: makeModel('0.3.5') })
    app.mount(host)
    await nextTick()

    const link = host.querySelector<HTMLAnchorElement>(
      'a[href="https://github.com/dongzhuo375/Hinina"]',
    )
    expect(link?.textContent).toContain('dongzhuo375/Hinina')
    expect(link?.querySelector('svg')).not.toBeNull()
    // 无 aria-label：可见 slug 天然成可访问名（WCAG 2.5.3 label-in-name）；
    // 重新加回覆盖型 aria-label 会让下方缺席断言变红
    expect(link?.hasAttribute('aria-label')).toBe(false)
    // 与 markdown 渲染面同款 fail-safe：拦截器失效时 wry 拒绝新窗口，
    // 点击无动作而非整页导航走
    expect(link?.getAttribute('target')).toBe('_blank')
    expect(link?.getAttribute('rel')).toBe('noopener noreferrer')
    // 版本号经 model 透传渲染（fixture 值非占位）
    expect(host.textContent).toContain('0.3.5')
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
