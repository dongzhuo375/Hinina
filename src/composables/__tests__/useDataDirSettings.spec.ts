import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { effectScope } from 'vue'
import type { EffectScope } from 'vue'
import { useDataDirSettings } from '@/composables/useDataDirSettings'

const mocks = vi.hoisted(() => ({
  getDataDir: vi.fn(),
  pickDataDir: vi.fn(),
  setDataDir: vi.fn(),
  resetDataDir: vi.fn(),
}))

vi.mock('@/services/system.service', () => ({
  systemService: {
    getDataDir: mocks.getDataDir,
    pickDataDir: mocks.pickDataDir,
    setDataDir: mocks.setDataDir,
    resetDataDir: mocks.resetDataDir,
  },
}))

let scope: EffectScope
let dataDir: ReturnType<typeof useDataDirSettings>

beforeEach(async () => {
  vi.resetAllMocks()
  vi.useFakeTimers()
  mocks.getDataDir.mockResolvedValue({
    currentDir: 'D:\\hinina', defaultDir: 'D:\\hinina', source: 'default', restartRequired: false,
  })
  mocks.pickDataDir.mockResolvedValue('D:\\new-data')
  mocks.setDataDir.mockResolvedValue(undefined)
  mocks.resetDataDir.mockResolvedValue(undefined)
  scope = effectScope()
  dataDir = scope.run(() => useDataDirSettings())!
  await dataDir.loadDataDir()
})

afterEach(() => {
  scope.stop()
  vi.useRealTimers()
  vi.restoreAllMocks()
})

describe('useDataDirSettings', () => {
  it('确认使用点击前的 migrate 快照，成功后复位并重读', async () => {
    await dataDir.chooseDataDir()
    expect(dataDir.pendingDir.value).toBe('D:\\new-data')
    dataDir.migrateData.value = false

    await dataDir.confirmDataDir()
    expect(mocks.setDataDir).toHaveBeenCalledWith('D:\\new-data', false)
    expect(dataDir.changeNotice.value).toContain('未迁移现有数据')
    expect(dataDir.pendingDir.value).toBeNull()
    expect(dataDir.migrateData.value).toBe(true)
    expect(mocks.getDataDir).toHaveBeenCalledTimes(2)
  })

  it('取消候选恢复 migrate=true，后续恢复默认不得静默不迁移', async () => {
    await dataDir.chooseDataDir()
    dataDir.migrateData.value = false
    dataDir.cancelDataDirChoice()
    expect(dataDir.pendingDir.value).toBeNull()
    expect(dataDir.migrateData.value).toBe(true)

    await dataDir.restoreDefaultDataDir()
    expect(mocks.resetDataDir).toHaveBeenCalledWith(true)
    expect(dataDir.changeNotice.value).toContain('迁移现有数据')
    expect(mocks.getDataDir).toHaveBeenCalledTimes(2)
  })
})
