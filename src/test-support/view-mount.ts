// 共享视图挂载 helper（M2 工作包 C）。
//
// 收敛既有 5 个 spec 各自复制的 createApp + pinia + settle 样板；仅服务测试，
// 不进生产构建路径（vitest include 只收 __tests__/**/*.spec.ts，本文件不会被执行）。
import { createApp, h, nextTick } from 'vue'
import type { App, Component } from 'vue'
import { createPinia, setActivePinia } from 'pinia'
import type { Pinia } from 'pinia'

export interface MountOptions {
  props?: Record<string, unknown>
  /** 注册到根 app 的组件桩（如 RouterView） */
  globalComponents?: Record<string, Component>
  /** mount 后的 nextTick 次数（默认 8，与既有 spec 的 settle 一致） */
  settleTicks?: number
}

export interface Mounted {
  app: App
  host: HTMLDivElement
  pinia: Pinia
  unmount: () => void
}

export async function mountView(
  component: Component,
  options: MountOptions = {},
): Promise<Mounted> {
  const host = document.createElement('div')
  document.body.append(host)
  const pinia = createPinia()
  setActivePinia(pinia)
  const app = createApp({ render: () => h(component, options.props ?? {}) })
  app.use(pinia)
  for (const [name, comp] of Object.entries(options.globalComponents ?? {})) {
    app.component(name, comp)
  }
  app.mount(host)
  for (let i = 0; i < (options.settleTicks ?? 8); i += 1) await nextTick()
  return {
    app,
    host,
    pinia,
    unmount: () => {
      app.unmount()
      host.remove()
    },
  }
}
