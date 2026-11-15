import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { effectScope } from 'vue'
import type { EffectScope } from 'vue'
import { useAboutSettings } from '@/composables/useAboutSettings'

const getStorageInfo = vi.hoisted(() => vi.fn())
vi.mock('@/services/system.service', () => ({ systemService: { getStorageInfo } }))

let scope: EffectScope
let about: ReturnType<typeof useAboutSettings>

beforeEach(() => {
  // composable 经 systemStore 转发到 service：需要活动 pinia
  setActivePinia(createPinia())
  vi.useFakeTimers()
  getStorageInfo.mockReset()
  getStorageInfo.mockResolvedValue({ baseDir: 'D:\\hinina', logPath: 'D:\\hinina\\log.txt', version: '0.1.2' })
  scope = effectScope()
  about = scope.run(() => useAboutSettings())!
})

afterEach(() => {
  scope.stop()
  vi.useRealTimers()
  vi.restoreAllMocks()
})

describe('useAboutSettings', () => {
  it('存储读取与局部失败互不影响', async () => {
    await about.loadStorage()
    expect(about.storage.value?.version).toBe('0.1.2')
    expect(about.storageFailed.value).toBe(false)

    getStorageInfo.mockRejectedValueOnce(new Error('io'))
    await about.loadStorage()
    expect(about.storageFailed.value).toBe(true)
  })

  it('失败后重载成功会复位 failed（useAsyncResource 的复位语义，可被重试路径依赖）', async () => {
    getStorageInfo.mockRejectedValueOnce(new Error('io'))
    await about.loadStorage()
    expect(about.storageFailed.value).toBe(true)

    // 重试成功：失败标志复位 + 数据写值（复位语义曾被复制走样，此用例锁死）
    await about.loadStorage()
    expect(about.storageFailed.value).toBe(false)
    expect(about.storage.value?.version).toBe('0.1.2')
  })

  it('复制使用共享 key 反馈并在 2 秒后回弹，失败保持静默', async () => {
    const writeText = vi.fn(async () => {})
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } })

    await about.copyText('version', '0.1.2')
    expect(writeText).toHaveBeenCalledWith('0.1.2')
    expect(about.copiedKey.value).toBe('version')
    vi.advanceTimersByTime(2_000)
    expect(about.copiedKey.value).toBeNull()

    writeText.mockRejectedValueOnce(new Error('denied'))
    await about.copyText('baseDir', 'D:\\hinina')
    expect(about.copiedKey.value).toBeNull()
  })
})
