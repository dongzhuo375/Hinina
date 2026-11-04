import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
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
  // composable 经 systemStore 转发到 service：需要活动 pinia
  setActivePinia(createPinia())
  vi.resetAllMocks()
  vi.useFakeTimers()
  mocks.getDataDir.mockResolvedValue({
    currentDir: 'D:\\hinina', defaultDir: 'D:\\hinina', source: 'default', restartRequired: false,
    pointerInTemp: false,
  })
  mocks.pickDataDir.mockResolvedValue('D:\\new-data')
  mocks.setDataDir.mockResolvedValue({
    targetDir: 'D:\\new-data', migrateFrom: null, restartRequired: true, warning: null,
  })
  mocks.resetDataDir.mockResolvedValue({
    targetDir: 'D:\\hinina', migrateFrom: null, restartRequired: true, warning: null,
  })
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

  /// 回归：降级模式下指针也保存在临时目录——后端下发的风险提示必须覆盖
  /// 「已记录」成功文案，否则用户以为已经换到安全位置。
  it('降级模式的风险提示替换普通成功文案', async () => {
    const warning = '当前处于临时目录降级模式：位置指针也保存在临时目录'
    mocks.setDataDir.mockResolvedValueOnce({
      targetDir: 'D:\\new-data', migrateFrom: null, restartRequired: true, warning,
    })
    await dataDir.chooseDataDir()

    await dataDir.confirmDataDir()

    expect(dataDir.changeNotice.value).toBe(warning)
  })
})
