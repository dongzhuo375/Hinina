import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { effectScope } from 'vue'
import type { EffectScope } from 'vue'
import { useMaintenanceSettings } from '@/composables/useMaintenanceSettings'

const mocks = vi.hoisted(() => ({
  resetClient: vi.fn(),
  localDataUsage: vi.fn(),
  purgeLocalData: vi.fn(),
  loadContest: vi.fn(),
  invalidateMyStatus: vi.fn(),
  announcementRefresh: vi.fn(),
}))

vi.mock('@/services/system.service', () => ({
  systemService: {
    resetClient: mocks.resetClient,
    localDataUsage: mocks.localDataUsage,
    purgeLocalData: mocks.purgeLocalData,
  },
}))
vi.mock('@/stores/contestStore', () => ({ useContestStore: () => ({ loadContest: mocks.loadContest }) }))
vi.mock('@/stores/problemStore', () => ({ useProblemStore: () => ({ invalidateMyStatus: mocks.invalidateMyStatus }) }))
vi.mock('@/stores/announcementStore', () => ({ useAnnouncementStore: () => ({ refresh: mocks.announcementRefresh }) }))

let scope: EffectScope
let maintenance: ReturnType<typeof useMaintenanceSettings>

beforeEach(() => {
  vi.resetAllMocks()
  vi.useFakeTimers()
  mocks.resetClient.mockResolvedValue(undefined)
  mocks.localDataUsage.mockResolvedValue({
    logBytes: 4096, logPath: 'D:\\hinina\\log.txt',
    crashBytes: 512, crashPath: 'C:\\Temp\\hinina-panic.log',
    snapshotTotalCount: 5,
    snapshotTotalBytes: 8192, snapshotStaleCount: 2, snapshotStaleBytes: 2048, keepDays: 30,
  })
  mocks.purgeLocalData.mockResolvedValue({ freedBytes: 2048, removedSnapshots: 2, logCleared: false })
  mocks.loadContest.mockResolvedValue(undefined)
  mocks.announcementRefresh.mockResolvedValue(undefined)
  scope = effectScope()
  maintenance = scope.run(() => useMaintenanceSettings())!
})

afterEach(() => {
  scope.stop()
  vi.useRealTimers()
  vi.restoreAllMocks()
})

describe('useMaintenanceSettings', () => {
  it('重置按原顺序补拉，公告 refresh 仍为 void 发出', async () => {
    await maintenance.resetClient()

    expect(mocks.resetClient).toHaveBeenCalledTimes(1)
    expect(mocks.loadContest).toHaveBeenCalledTimes(1)
    expect(mocks.invalidateMyStatus).toHaveBeenCalledTimes(1)
    expect(mocks.announcementRefresh).toHaveBeenCalledTimes(1)
    expect(maintenance.resetNotice.value).toEqual({ text: '已重置，数据已重新拉取', warn: false })
  })

  it('重置本身失败时不触发任何补拉', async () => {
    mocks.resetClient.mockRejectedValueOnce(new Error('io'))
    await maintenance.resetClient()

    expect(maintenance.resetError.value).toBe('io')
    expect(mocks.loadContest).not.toHaveBeenCalled()
    expect(mocks.invalidateMyStatus).not.toHaveBeenCalled()
    expect(mocks.announcementRefresh).not.toHaveBeenCalled()
  })

  it('清理双空拦截；勾选日志但未清掉时如实 warning 并重读占用', async () => {
    maintenance.purgeLogs.value = false
    maintenance.purgeSnapshots.value = false
    await maintenance.purgeLocalData()
    expect(mocks.purgeLocalData).not.toHaveBeenCalled()
    expect(maintenance.purgeError.value).toBe('请先选择要清理的内容')

    maintenance.purgeLogs.value = true
    maintenance.purgeSnapshots.value = true
    await maintenance.purgeLocalData()
    expect(maintenance.purgeNotice.value?.warn).toBe(true)
    expect(maintenance.purgeNotice.value?.text).toContain('日志或崩溃记录未清理')
    expect(mocks.localDataUsage).toHaveBeenCalledTimes(1)
  })
})
