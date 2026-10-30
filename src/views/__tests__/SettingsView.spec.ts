// @vitest-environment jsdom
// SettingsView 维护区（重置/清理）关键流测试（M2 工作包 C，范围刻意收窄：
// 只测「不可逆操作的如实反馈」—— 双空勾选拦截、日志未清掉的 warn 文案、
// 重置后补拉失败的 warn 文案。其余区块的 mock 面过大，留待后续按需补充。
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mountView } from '@/test-support/view-mount'
import type { Mounted } from '@/test-support/view-mount'
import type { AppConfig } from '@/types/config'

const mocks = vi.hoisted(() => ({
  // system.service
  resetClient: vi.fn(async () => {}),
  localDataUsage: vi.fn(async () => ({
    logBytes: 4096, logPath: 'D:\\hinina\\logs\\hinina.log',
    crashBytes: 512, crashPath: 'C:\\Temp\\hinina-panic.log',
    snapshotTotalCount: 5, snapshotTotalBytes: 8192,
    snapshotStaleCount: 2, snapshotStaleBytes: 2048, keepDays: 30,
  })),
  purgeLocalData: vi.fn(async () => ({ freedBytes: 0, removedSnapshots: 0, logCleared: false, crashCleared: true })),
  getStorageInfo: vi.fn(async () => ({ baseDir: 'D:\\hinina', logPath: 'D:\\hinina\\logs\\hinina.log', version: '0.1.0' })),
  getDataDir: vi.fn(async () => ({ currentDir: 'D:\\hinina', defaultDir: 'D:\\hinina', source: 'default', restartRequired: false })),
  // config.service
  getConfig: vi.fn(async () => ({})),
  updateConfig: vi.fn(async () => {}),
  invalidate: vi.fn(),
  // stores
  loadContest: vi.fn(async () => {}),
  invalidateMyStatus: vi.fn(),
  announcementRefresh: vi.fn(async () => {}),
  resetSessionForOjSwitch: vi.fn(),
  routerReplace: vi.fn(),
}))

vi.mock('@/services/system.service', () => ({
  systemService: {
    resetClient: mocks.resetClient,
    localDataUsage: mocks.localDataUsage,
    purgeLocalData: mocks.purgeLocalData,
    getStorageInfo: mocks.getStorageInfo,
    getDataDir: mocks.getDataDir,
  },
}))
vi.mock('@/services/config.service', () => ({
  configService: { getConfig: mocks.getConfig, updateConfig: mocks.updateConfig, invalidate: mocks.invalidate },
}))
vi.mock('@/stores/contestStore', () => ({ useContestStore: () => ({ loadContest: mocks.loadContest }) }))
vi.mock('@/stores/problemStore', () => ({ useProblemStore: () => ({ invalidateMyStatus: mocks.invalidateMyStatus }) }))
vi.mock('@/stores/announcementStore', () => ({ useAnnouncementStore: () => ({ refresh: mocks.announcementRefresh }) }))
vi.mock('@/stores/session', () => ({ resetSessionForOjSwitch: mocks.resetSessionForOjSwitch }))
vi.mock('vue-router', () => ({ useRouter: () => ({ replace: mocks.routerReplace }) }))
vi.mock('@/components/common/PageHeading.vue', () => ({ default: { template: '<header />' } }))
vi.mock('@/components/common/ErrorMessage.vue', () => ({ default: { template: '<div />' } }))
vi.mock('@/components/common/LoadingSpinner.vue', () => ({ default: { template: '<div />' } }))

import SettingsView from '@/views/SettingsView.vue'

const baseConfig = (): AppConfig => ({
  user: { lastUsername: '' },
  oj: {
    active: 'HOJ',
    instances: [{ id: 'HOJ', baseUrl: 'https://hoj.example', enabled: true, options: {} }],
    contestRef: '1001',
    contestPassword: null,
    timeoutSecs: 30,
    pollIntervalSecs: 5,
    pollTimeoutSecs: 300,
    cacheTtlSecs: 60,
    cacheProblemStatement: true,
  },
  editor: { fontSize: 14, tabSize: 4, autoSave: true, autoSaveIntervalSecs: 30, defaultLanguage: 'C++' },
  theme: { themeName: 'light', editorTheme: 'vs' },
  layout: { sidebarWidth: 240, splitRatio: 0.48 },
})

/// 登记挂载实例，afterEach 统一卸载（评审第三轮 #10）：失败用例不残留挂载
/// 实例，避免后续用例经全局 document 查询误触旧实例产生连带失败。
const mounted: Mounted[] = []
async function render() {
  mocks.getConfig.mockImplementation(async () => baseConfig())
  const m = await mountView(SettingsView)
  mounted.push(m)
  return m
}

/// 按标签文本找复选框并改选（v-model checkbox 走 change 事件）。
function setCheckbox(labelText: string, checked: boolean) {
  const label = [...document.querySelectorAll('label')].find(l =>
    l.textContent?.includes(labelText),
  )
  expect(label).toBeTruthy() // 应找到标签
  const input = label!.querySelector('input[type="checkbox"]') as HTMLInputElement
  input.checked = checked
  input.dispatchEvent(new Event('change'))
}

function clickButton(text: string) {
  const btn = [...document.querySelectorAll('button')].find(b => b.textContent?.includes(text))
  expect(btn).toBeTruthy() // 应找到按钮
  btn!.click()
}

const settle = async () => {
  for (let i = 0; i < 12; i += 1) await Promise.resolve()
}

beforeEach(() => {
  vi.clearAllMocks()
})

afterEach(() => {
  while (mounted.length > 0) mounted.pop()?.unmount()
})

describe('SettingsView 清理本地数据（不可逆，反馈必须如实）', () => {
  it('两个勾选全空：不发清理请求，给出「请先选择」错误', async () => {
    await render()
    await settle()
    setCheckbox('日志与崩溃记录', false)
    setCheckbox('提交源码留档', false)
    await settle()

    clickButton('清理本地数据')
    await settle()
    clickButton('确认清理（不可恢复）')
    await settle()

    expect(mocks.purgeLocalData).not.toHaveBeenCalled() // 全空勾选必须在前端拦截
    expect(document.body.textContent).toContain('请先选择要清理的内容')
  })

  it('勾了日志但后端没清掉（logCleared=false）：warn 文案如实说明', async () => {
    mocks.purgeLocalData.mockResolvedValueOnce({ freedBytes: 2048, removedSnapshots: 2, logCleared: false, crashCleared: true })
    await render()
    await settle()

    clickButton('清理本地数据')
    await settle()
    clickButton('确认清理（不可恢复）')
    await settle()

    expect(mocks.purgeLocalData).toHaveBeenCalledWith(true, true)
    expect(document.body.textContent).toContain('日志未清理') // 不能笼统报已清理
    expect(mocks.localDataUsage).toHaveBeenCalledTimes(2) // 清理后必须重读占用预览
  })
})

describe('SettingsView 重置客户端', () => {
  it('重置成功但补拉失败：提示必须带 warn 语义（如实，不宣称数据已最新）', async () => {
    mocks.loadContest.mockRejectedValueOnce(new Error('net'))
    await render()
    await settle()

    clickButton('重置客户端')
    await settle()
    clickButton('确认重置')
    await settle()

    expect(mocks.resetClient).toHaveBeenCalledTimes(1)
    expect(mocks.invalidateMyStatus).toHaveBeenCalledTimes(1) // 我的题目状态标记过期
    expect(mocks.announcementRefresh).toHaveBeenCalledTimes(1) // 公告与已读必须补拉
    expect(document.body.textContent).toContain('数据重新拉取失败')
  })
})
