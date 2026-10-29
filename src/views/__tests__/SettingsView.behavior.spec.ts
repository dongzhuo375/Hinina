// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mountView, settleView } from '@/test-support/view-mount'
import type { Mounted } from '@/test-support/view-mount'
import type { AppConfig } from '@/types/config'

const mocks = vi.hoisted(() => ({
  getConfig: vi.fn(),
  updateConfig: vi.fn(),
  invalidate: vi.fn(),
  switchOj: vi.fn(),
  getStorageInfo: vi.fn(),
  localDataUsage: vi.fn(),
  getDataDir: vi.fn(),
  pickDataDir: vi.fn(),
  setDataDir: vi.fn(),
  resetDataDir: vi.fn(),
  resetClient: vi.fn(),
  purgeLocalData: vi.fn(),
  loadContest: vi.fn(),
  invalidateMyStatus: vi.fn(),
  announcementRefresh: vi.fn(),
  resetSessionForOjSwitch: vi.fn(),
  routerReplace: vi.fn(),
}))

vi.mock('@/services/config.service', () => ({
  configService: {
    getConfig: mocks.getConfig,
    updateConfig: mocks.updateConfig,
    invalidate: mocks.invalidate,
    switchOj: mocks.switchOj,
  },
}))
vi.mock('@/services/system.service', () => ({
  systemService: {
    getStorageInfo: mocks.getStorageInfo,
    localDataUsage: mocks.localDataUsage,
    getDataDir: mocks.getDataDir,
    pickDataDir: mocks.pickDataDir,
    setDataDir: mocks.setDataDir,
    resetDataDir: mocks.resetDataDir,
    resetClient: mocks.resetClient,
    purgeLocalData: mocks.purgeLocalData,
  },
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

function baseConfig(): AppConfig {
  return {
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
  }
}

let currentConfig: AppConfig
const mounted: Mounted[] = []

async function render(): Promise<Mounted> {
  const view = await mountView(SettingsView)
  mounted.push(view)
  return view
}

function field(host: HTMLElement, labelText: string): HTMLInputElement | HTMLSelectElement {
  const label = [...host.querySelectorAll('label')].find((item) => {
    const title = item.querySelector(':scope > span')?.textContent ?? item.textContent
    return title?.includes(labelText)
  })
  expect(label, `缺少字段 ${labelText}`).toBeTruthy()
  return label!.querySelector('input, select') as HTMLInputElement | HTMLSelectElement
}

async function setField(host: HTMLElement, labelText: string, value: string): Promise<void> {
  const input = field(host, labelText)
  input.value = value
  input.dispatchEvent(new Event(input instanceof HTMLSelectElement ? 'change' : 'input', { bubbles: true }))
}

function button(host: HTMLElement, text: string): HTMLButtonElement {
  const found = [...host.querySelectorAll('button')].find((item) => item.textContent?.includes(text))
  expect(found, `缺少按钮 ${text}`).toBeTruthy()
  return found!
}

function setCheckbox(host: HTMLElement, labelText: string, checked: boolean): void {
  const input = field(host, labelText) as HTMLInputElement
  input.checked = checked
  input.dispatchEvent(new Event('change', { bubbles: true }))
}

beforeEach(() => {
  vi.resetAllMocks()
  currentConfig = baseConfig()
  mocks.getConfig.mockImplementation(async () => structuredClone(currentConfig))
  mocks.updateConfig.mockImplementation(async (mutator: (draft: AppConfig) => void) => {
    const draft = structuredClone(currentConfig)
    mutator(draft)
    currentConfig = draft
    return draft
  })
  mocks.switchOj.mockResolvedValue(undefined)
  mocks.getStorageInfo.mockResolvedValue({
    baseDir: 'D:\\hinina',
    logPath: 'D:\\hinina\\logs\\hinina.log',
    version: '0.1.2',
  })
  mocks.localDataUsage.mockResolvedValue({
    logBytes: 4096,
    logPath: 'D:\\hinina\\logs\\hinina.log',
    snapshotTotalCount: 5,
    snapshotTotalBytes: 8192,
    snapshotStaleCount: 2,
    snapshotStaleBytes: 2048,
    keepDays: 30,
  })
  mocks.getDataDir.mockResolvedValue({
    currentDir: 'D:\\hinina',
    defaultDir: 'D:\\hinina',
    source: 'default',
    restartRequired: false,
  })
  mocks.pickDataDir.mockResolvedValue('D:\\new-data')
  mocks.setDataDir.mockResolvedValue(undefined)
  mocks.resetDataDir.mockResolvedValue(undefined)
  mocks.resetClient.mockResolvedValue(undefined)
  mocks.purgeLocalData.mockResolvedValue({ freedBytes: 0, removedSnapshots: 0, logCleared: true })
  mocks.loadContest.mockResolvedValue(undefined)
  mocks.announcementRefresh.mockResolvedValue(undefined)
})

afterEach(() => {
  while (mounted.length > 0) mounted.pop()?.unmount()
  vi.useRealTimers()
  vi.restoreAllMocks()
})

describe('SettingsView 行为表征（结构拆分前）', () => {
  it('配置仍在加载时也按原顺序启动其余三路读取', async () => {
    mocks.getConfig.mockImplementation(() => new Promise<AppConfig>(() => {}))
    await render()

    expect(mocks.invalidate).toHaveBeenCalledTimes(1)
    expect(mocks.getConfig).toHaveBeenCalledTimes(1)
    expect(mocks.getStorageInfo).toHaveBeenCalledTimes(1)
    expect(mocks.localDataUsage).toHaveBeenCalledTimes(1)
    expect(mocks.getDataDir).toHaveBeenCalledTimes(1)
    expect(mocks.invalidate.mock.invocationCallOrder[0]).toBeLessThan(mocks.getConfig.mock.invocationCallOrder[0])
    expect(mocks.getConfig.mock.invocationCallOrder[0]).toBeLessThan(mocks.getStorageInfo.mock.invocationCallOrder[0])
    expect(mocks.getStorageInfo.mock.invocationCallOrder[0]).toBeLessThan(mocks.localDataUsage.mock.invocationCallOrder[0])
    expect(mocks.localDataUsage.mock.invocationCallOrder[0]).toBeLessThan(mocks.getDataDir.mock.invocationCallOrder[0])
  })

  it('七个真实 section 的 id 与导航顺序保持不变', async () => {
    const view = await render()
    const ids = [...view.host.querySelectorAll('section.settings-section')].map((section) => section.id)
    expect(ids).toEqual(Array.from({ length: 7 }, (_, index) => `settings-${index}`))
    const hrefs = [...view.host.querySelectorAll('.settings-index a')].map((link) => link.getAttribute('href'))
    expect(hrefs).toEqual(Array.from({ length: 7 }, (_, index) => `#settings-${index}`))
  })

  it('真实编辑后保存完整配置：不得写 active，成功后推进 baseline', async () => {
    const view = await render()
    await setField(view.host, '比赛 ID / 引用', ' 2002 ')
    button(view.host, '保存').click()
    await vi.waitFor(() => expect(mocks.updateConfig).toHaveBeenCalledTimes(1))
    await settleView(2)

    expect(currentConfig.oj.contestRef).toBe('2002')
    expect(currentConfig.oj.contestPassword).toBeNull()
    expect(currentConfig.oj.active).toBe('HOJ')
    expect(currentConfig.editor.fontSize).toBe(14)
    expect(currentConfig.layout.splitRatio).toBe(0.48)
    expect(mocks.switchOj).not.toHaveBeenCalled()
    expect(view.host.textContent).toContain('已保存')
    expect(button(view.host, '保存').disabled).toBe(true)
  })

  it('未配置 OJ：保存创建实例后自动切换，失败不得清会话或导航', async () => {
    const view = await render()
    await setField(view.host, '当前 OJ', 'Hydro')
    expect(mocks.switchOj).not.toHaveBeenCalled()
    expect(field(view.host, '服务器地址').value).toBe('')
    await setField(view.host, '服务器地址', 'https://hydro.example')
    button(view.host, '保存').click()
    await vi.waitFor(() => expect(mocks.switchOj).toHaveBeenCalledWith('Hydro'))
    await settleView(2)

    expect(currentConfig.oj.instances).toContainEqual({
      id: 'Hydro', baseUrl: 'https://hydro.example', enabled: true, options: {},
    })
    expect(mocks.resetSessionForOjSwitch).toHaveBeenCalledTimes(1)
    expect(mocks.routerReplace).toHaveBeenCalledWith({ name: 'Login' })
  })

  it('非法数字实时显示字段级错误，修正后消失', async () => {
    const view = await render()
    await setField(view.host, '请求超时（秒）', 'abc')
    await settleView(2)
    expect(view.host.textContent).toContain('请求超时须为非负整数')

    await setField(view.host, '请求超时（秒）', '30')
    await settleView(2)
    expect(view.host.textContent).not.toContain('请求超时须为非负整数')
  })

  it('新实例保存后下拉 configured 标签随实例清单更新', async () => {
    mocks.switchOj.mockRejectedValueOnce(new Error('切换失败'))
    const view = await render()
    await setField(view.host, '当前 OJ', 'Hydro')
    expect((field(view.host, '当前 OJ') as HTMLSelectElement).selectedOptions[0]?.textContent)
      .toContain('未配置')

    await setField(view.host, '服务器地址', 'https://hydro.example')
    button(view.host, '保存').click()
    await vi.waitFor(() => expect(mocks.switchOj).toHaveBeenCalledWith('Hydro'))
    await settleView(3)
    const select = field(view.host, '当前 OJ') as HTMLSelectElement
    const option = [...select.options].find((item) => item.value === 'Hydro')
    expect(option?.textContent).not.toContain('未配置')
    expect(select.value).toBe('HOJ')
    expect(field(view.host, '服务器地址').value).toBe('https://hoj.example')
    expect(button(view.host, '保存').disabled).toBe(true)
    expect(view.host.textContent).not.toContain('有未保存的更改')

    // 同一下拉重新选择 Hydro 即可重试切换
    await setField(view.host, '当前 OJ', 'Hydro')
    await vi.waitFor(() => expect(mocks.switchOj).toHaveBeenCalledTimes(2))
  })

  it('已配置 OJ 切换失败：回滚下拉且不清会话、不导航', async () => {
    currentConfig.oj.instances.push({ id: 'Hydro', baseUrl: 'https://hydro.example', enabled: true, options: {} })
    mocks.switchOj.mockRejectedValueOnce(new Error('切换失败'))
    const view = await render()

    await setField(view.host, '当前 OJ', 'Hydro')
    await vi.waitFor(() => expect(mocks.switchOj).toHaveBeenCalledWith('Hydro'))
    await settleView(2)

    expect(field(view.host, '当前 OJ').value).toBe('HOJ')
    await vi.waitFor(() => expect(view.host.textContent).toContain('切换失败'))
    expect(mocks.resetSessionForOjSwitch).not.toHaveBeenCalled()
    expect(mocks.routerReplace).not.toHaveBeenCalled()
  })

  it('数据目录确认使用点击前的 migrate 快照，并在成功后重读', async () => {
    const view = await render()
    button(view.host, '更改目录').click()
    await vi.waitFor(() => expect(mocks.pickDataDir).toHaveBeenCalledTimes(1))
    await settleView(2)
    setCheckbox(view.host, '把现有数据迁移到新目录', false)
    button(view.host, '确认更改').click()
    await vi.waitFor(() => expect(mocks.setDataDir).toHaveBeenCalledWith('D:\\new-data', false))
    await settleView(2)

    expect(view.host.textContent).toContain('未迁移现有数据')
    expect(mocks.getDataDir).toHaveBeenCalledTimes(2)
  })

  it('About 复制只反馈当前 key，2 秒后回弹', async () => {
    vi.useFakeTimers()
    const writeText = vi.fn(async () => {})
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } })
    const view = await render()
    const about = view.host.querySelector('#settings-6')!
    const copyButton = about.querySelector('button')!

    copyButton.click()
    await vi.waitFor(() => expect(writeText).toHaveBeenCalledWith('0.1.2'))
    await settleView(2)
    expect(copyButton.textContent).toContain('已复制')
    await vi.advanceTimersByTimeAsync(2_000)
    expect(about.textContent).not.toContain('已复制')
  })

  it('scroll-spy 点击锁定、超时释放，滚到底选择最后一组', async () => {
    vi.useFakeTimers()
    const view = await render()
    const scroller = view.host.querySelector<HTMLElement>('.overflow-y-auto')!
    Object.defineProperty(scroller, 'scrollHeight', { configurable: true, value: 2_000 })
    Object.defineProperty(scroller, 'clientHeight', { configurable: true, value: 500 })
    scroller.getBoundingClientRect = () => ({ top: 0 } as DOMRect)
    const sections = Array.from({ length: 7 }, (_, index) => {
      const element = view.host.querySelector<HTMLElement>(`#settings-${index}`)!
      element.getBoundingClientRect = () => ({ top: index * 300 } as DOMRect)
      element.scrollIntoView = vi.fn()
      element.focus = vi.fn()
      return element
    })
    scroller.dispatchEvent(new Event('scroll'))
    await vi.advanceTimersByTimeAsync(0)

    const aboutLink = view.host.querySelector<HTMLAnchorElement>('a[href="#settings-6"]')!
    aboutLink.click()
    await settleView(2)
    expect(aboutLink.getAttribute('aria-current')).toBe('true')
    expect(sections[6].scrollIntoView).toHaveBeenCalledWith({ block: 'start', behavior: 'smooth' })
    expect(sections[6].focus).toHaveBeenCalledWith({ preventScroll: true })

    await vi.advanceTimersByTimeAsync(1_000)
    scroller.dispatchEvent(new Event('scroll'))
    await vi.advanceTimersByTimeAsync(0)
    await settleView(2)
    // 1s 后平滑滚动可能仍在继续，锁不得提前释放
    expect(aboutLink.getAttribute('aria-current')).toBe('true')

    scroller.dispatchEvent(new Event('scrollend'))
    await settleView(2)
    expect(view.host.querySelector('a[href="#settings-0"]')?.getAttribute('aria-current')).toBe('true')

    scroller.scrollTop = 1_500
    scroller.dispatchEvent(new Event('scroll'))
    await vi.advanceTimersByTimeAsync(0)
    await settleView(2)
    expect(aboutLink.getAttribute('aria-current')).toBe('true')
  })
})
