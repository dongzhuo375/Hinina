import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

/// Store 只是 UI 消费入口：状态仍在 service（本测试只钉转发与回传）。
const mocks = vi.hoisted(() => ({
  launchFlags: { value: { settings: false } },
  minimizeWindow: vi.fn(),
  closeWindow: vi.fn(),
  getStorageInfo: vi.fn(),
  setDataDir: vi.fn(),
  onCloseRequested: vi.fn(),
}))
vi.mock('@/services/system.service', () => ({ systemService: { ...mocks } }))

import { useSystemStore } from '@/stores/systemStore'

describe('systemStore — UI 消费入口', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    for (const fn of Object.values(mocks)) if (vi.isMockFunction(fn)) fn.mockReset()
  })

  it('isKeyed 读响应式启动标志快照（非方法调用，避免 getter 缓存陈旧值）', () => {
    mocks.launchFlags.value = { settings: true }
    expect(useSystemStore().isKeyed).toBe(true)
    // 新 store 实例（同一快照）：false 也能即时反映
    mocks.launchFlags.value = { settings: false }
    setActivePinia(createPinia())
    expect(useSystemStore().isKeyed).toBe(false)
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
