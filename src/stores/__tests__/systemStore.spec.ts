import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

/// Store 只是 UI 消费入口：状态仍在 service（本测试只钉转发与回传）。
/// 启动标志用**真实响应式 ref** 打桩：验证 getter 经 `systemService.isKeyed()`
/// 方法调用也能建立依赖（Pinia getter 是 computed，方法内部读 ref 会被追踪）。
const mocks = vi.hoisted(() => ({
  minimizeWindow: vi.fn(),
  closeWindow: vi.fn(),
  getStorageInfo: vi.fn(),
  setDataDir: vi.fn(),
}))

vi.mock('@/services/system.service', async () => {
  const { ref } = await import('vue')
  const launchFlags = ref<{ settings: boolean }>({ settings: false })
  return {
    systemService: {
      ...mocks,
      launchFlags,
      isKeyed: () => launchFlags.value.settings,
    },
    __launchFlags: launchFlags,
  }
})

import * as systemModule from '@/stores/../services/system.service'
import { useSystemStore } from '@/stores/systemStore'

const launchFlags = (
  systemModule as unknown as {
    __launchFlags: { value: { settings: boolean } }
  }
).__launchFlags

describe('systemStore — UI 消费入口', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    for (const fn of Object.values(mocks)) fn.mockReset()
    launchFlags.value = { settings: false }
  })

  it('isKeyed 转发单一判据，并随启动标志快照重新求值', () => {
    launchFlags.value = { settings: true }
    const store = useSystemStore()
    expect(store.isKeyed).toBe(true)
    // 同一个 store 实例：快照变化后 getter 必须重新求值（依赖经方法调用追踪）
    launchFlags.value = { settings: false }
    expect(store.isKeyed).toBe(false)
  })

  it('窗口控制转发到 service', async () => {
    mocks.minimizeWindow.mockResolvedValue(undefined)
    mocks.closeWindow.mockResolvedValue(undefined)
    const store = useSystemStore()
    await store.minimizeWindow()
    await store.closeWindow()
    expect(mocks.minimizeWindow).toHaveBeenCalledTimes(1)
    expect(mocks.closeWindow).toHaveBeenCalledTimes(1)
  })

  it('设置页动作转发并原样回传结果', async () => {
    const info = { baseDir: 'D:\hinina', logPath: 'D:\hinina\log.txt', version: '0.1.2' }
    mocks.getStorageInfo.mockResolvedValue(info)
    const change = { targetDir: 'D:\new', migrateFrom: null, restartRequired: true, warning: null }
    mocks.setDataDir.mockResolvedValue(change)

    const store = useSystemStore()
    await expect(store.getStorageInfo()).resolves.toEqual(info)
    await expect(store.setDataDir('D:\new', true)).resolves.toEqual(change)
    expect(mocks.setDataDir).toHaveBeenCalledWith('D:\new', true)
  })
})
