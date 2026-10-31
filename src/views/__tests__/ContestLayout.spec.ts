// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, nextTick } from 'vue'
import type { App } from 'vue'
import { createPinia, setActivePinia } from 'pinia'
import { useContestStore } from '@/stores/contestStore'
import { useAuthStore } from '@/stores/authStore'
import type { Contest } from '@/types/contest'
import ContestLayout from '@/views/ContestLayout.vue'

const mocks = vi.hoisted(() => ({
  replace: vi.fn(),
  startLive: vi.fn(),
  stopLive: vi.fn(),
  loadConfiguredContest: vi.fn(),
}))
vi.mock('vue-router', () => ({ useRouter: () => ({ replace: mocks.replace }) }))
vi.mock('@/stores/announcementStore', () => ({ useAnnouncementStore: () => mocks }))
vi.mock('@/services/contest.service', () => ({ contestService: { loadConfiguredContest: mocks.loadConfiguredContest } }))
vi.mock('@/components/layout/TopBar.vue', () => ({ default: { template: '<header />' } }))
vi.mock('@/components/layout/StatusBar.vue', () => ({ default: { template: '<footer />' } }))
vi.mock('@/components/layout/ActivityBar.vue', () => ({ default: { template: '<aside />' } }))

const schedule: Contest = {
  id: '1', title: '竞赛', startTime: 900, endTime: 1_002,
  description: '', contestType: 0, status: 0, auth: 0,
  rankShowName: 'username', sealRank: false, sealRankTime: null,
  allowEndSubmit: false, oiRankScoreType: null,
}
let app: App | undefined
let host: HTMLDivElement

async function render(contest: Contest = schedule, brief?: Contest) {
  const pinia = createPinia()
  setActivePinia(pinia)
  const store = useContestStore()
  store.contest = { ...contest }
  if (brief) store.brief = { ...brief }
  app = createApp(ContestLayout)
  app.use(pinia)
  app.component('RouterView', { template: '<div>contest content</div>' })
  app.mount(host)
  for (let i = 0; i < 5; i++) await nextTick()
  return store
}

beforeEach(() => {
  vi.useFakeTimers()
  vi.setSystemTime(1_000_000)
  host = document.createElement('div')
  document.body.append(host)
})

afterEach(() => {
  app?.unmount()
  app = undefined
  host.remove()
  vi.useRealTimers()
})

describe('contest exit lifecycle', () => {
  it('shows the end notice at the scheduled end, stops announcements and returns after 2 seconds', async () => {
    await render()
    expect(mocks.startLive).toHaveBeenCalledOnce()
    await vi.advanceTimersByTimeAsync(2_000)
    expect(host.querySelector('[role="alert"]')!.textContent).toContain('比赛已结束')
    expect(host.querySelector('[role="alert"]')!.parentElement?.classList.contains('top-10')).toBe(true)
    expect(host.querySelector('[inert]')).not.toBeNull()
    expect(mocks.stopLive).toHaveBeenCalledOnce()
    expect(mocks.replace).not.toHaveBeenCalled()
    await vi.advanceTimersByTimeAsync(2_000)
    expect(mocks.replace).toHaveBeenCalledExactlyOnceWith({ name: 'Login' })
  })

  it('also exits when the server ends a contest early', async () => {
    const store = await render({ ...schedule, endTime: 2_000 })
    store.contest!.status = 1
    await nextTick()
    expect(host.textContent).toContain('比赛已结束')
    await vi.advanceTimersByTimeAsync(2_000)
    expect(mocks.replace).toHaveBeenCalledExactlyOnceWith({ name: 'Login' })
  })

  it('returns outside if the authoritative schedule is moved into the future', async () => {
    await render({ ...schedule, startTime: 1_500, endTime: 2_000 })
    expect(mocks.replace).toHaveBeenCalledExactlyOnceWith({ name: 'Login' })
    expect(mocks.startLive).not.toHaveBeenCalled()
  })

  it('uses the current brief instead of an ended contest cached under another ID', async () => {
    const store = await render({ ...schedule, id: 'old', status: 1 })
    store.brief = { ...schedule, id: 'current', status: 0 }
    await nextTick()

    expect(host.querySelector('[role="alert"]')).toBeNull()
    expect(mocks.replace).not.toHaveBeenCalled()
  })

  it('refreshes authenticated contest data when the brief points to a new contest', async () => {
    const refreshed = { ...schedule, id: 'current', title: '新比赛', status: 0 }
    mocks.loadConfiguredContest.mockResolvedValue({ contest: refreshed, problems: [] })

    const store = await render(
      { ...schedule, id: 'old', title: '旧比赛', status: 1 },
      { ...schedule, id: 'current', title: '新比赛', status: 0 },
    )
    await Promise.resolve()
    await nextTick()

    expect(mocks.loadConfiguredContest).toHaveBeenCalledOnce()
    expect(store.contest).toEqual(refreshed)
    expect(mocks.startLive).toHaveBeenCalledExactlyOnceWith('current', expect.any(Function))
  })

  it('cancels pending exit and clock timers when leaving', async () => {
    await render({ ...schedule, status: 1 })
    app!.unmount()
    app = undefined
    await vi.advanceTimersByTimeAsync(5_000)
    expect(mocks.replace).not.toHaveBeenCalled()
    expect(vi.getTimerCount()).toBe(0)
  })
})

describe('--settings 无会话最小壳', () => {
  it('无已解析会话时渲染最小壳：不出顶栏/状态条，不发起认证拉取，返回登录可用', async () => {
    const pinia = createPinia()
    setActivePinia(pinia)
    // 会话已判定但不存在（user=null）：--settings 直落设置页时的外壳形态
    useAuthStore().sessionResolved = true
    app = createApp(ContestLayout)
    app.use(pinia)
    app.component('RouterView', { template: '<div>settings content</div>' })
    app.mount(host)
    for (let i = 0; i < 5; i++) await nextTick()

    expect(host.textContent).toContain('Hinina 设置')
    // 裸壳自带拖动区标题栏（decorations=false 下窗口移动全靠它）
    expect(host.querySelector('header[data-tauri-drag-region]')).toBeTruthy()
    expect(host.querySelector('header')).toBeTruthy()
    // bootstrap 必须跳过：无会话时认证拉取只会失败
    expect(mocks.loadConfiguredContest).not.toHaveBeenCalled()
    expect(mocks.startLive).not.toHaveBeenCalled()

    const back = [...host.querySelectorAll('button')].find((b) =>
      b.textContent?.includes('返回登录'),
    )
    expect(back).toBeTruthy()
    back!.click()
    await nextTick()
    expect(mocks.replace).toHaveBeenCalledWith({ name: 'Login' })
  })
})
