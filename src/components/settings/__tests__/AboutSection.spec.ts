// @vitest-environment jsdom
import { createApp, nextTick, ref } from 'vue'
import { describe, expect, it } from 'vitest'
import type { StorageInfo } from '@/types/system'
import AboutSection from '@/components/settings/AboutSection.vue'
import { useAboutSettings } from '@/composables/useAboutSettings'

/// 组件只消费模型形状：以同形 refs 直建模型，不经 pinia/IPC
function makeModel(storage: StorageInfo | null) {
  return {
    storage: ref<StorageInfo | null>(storage),
    storageFailed: ref(false),
    loadStorage: async () => {},
    copiedKey: ref<string | null>(null),
    copyText: async () => {},
  }
}

describe('AboutSection', () => {
  it('在存储信息之下渲染项目仓库外链（GitHub 图标 + 仓库 slug）', async () => {
    const host = document.createElement('div')
    const app = createApp(AboutSection, {
      model: makeModel({
        version: '0.3.5',
        baseDir: 'C:\\data\\hinina',
        logPath: 'C:\\data\\hinina\\logs\\hinina.log',
      }),
    })

    app.mount(host)
    await nextTick()

    const link = host.querySelector<HTMLAnchorElement>(
      'a[href="https://github.com/dongzhuo375/Hinina"]',
    )
    expect(link).not.toBeNull()
    expect(link?.getAttribute('aria-label')).toBe('在 GitHub 上查看项目仓库')
    // GitHub 图标为内联 SVG（离线约束：不引入图标字体）
    expect(link?.querySelector('svg')).not.toBeNull()
    expect(link?.textContent).toContain('dongzhuo375/Hinina')
    // 打开动作由 document 级外链拦截器转交系统浏览器：普通锚点即可，不依赖 target
    expect(link?.hasAttribute('target')).toBe(false)

    app.unmount()
  })

  it('存储信息获取失败时仓库行仍然可见', async () => {
    const host = document.createElement('div')
    const model = makeModel(null)
    model.storageFailed.value = true
    const app = createApp(AboutSection, { model })

    app.mount(host)
    await nextTick()

    expect(host.textContent).toContain('获取失败')
    expect(
      host.querySelector('a[href="https://github.com/dongzhuo375/Hinina"]'),
    ).not.toBeNull()

    app.unmount()
  })
})
