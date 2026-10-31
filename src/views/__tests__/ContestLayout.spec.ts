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
  // 当前路由名（最小壳判据的一部分）；默认非 Settings，模拟工作台子路由
  routeName: { value: 'ProblemSet' as string | undefined },
  // 启动标志响应式快照（systemService.launchFlags）：最小壳判据之一
  launchFlags: { value: { settings: false } },
}))
vi.mock('vue-router', () => ({
  useRouter: () => ({ replace: mocks.replace }),
  useRoute: () => ({ name: mocks.routeName.value }),
}))
vi.mock('@/services/system.service', () => ({
  systemService: { launchFlags: mocks.launchFlags },
}))
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
  it('无会话且当前路由是设置页：渲染最小壳，不出顶栏/状态条，不发起认证拉取，返回登录可用', async () => {
    const pinia = createPinia()
    setActivePinia(pinia)
    // 钥匙启动 + 无会话 + 设置路由。刻意不设置 sessionResolved：bareShell 不读它
    //（评审四 P2-1）——会话失效清理会把它复位为 false，最小壳必须照常接管
    mocks.launchFlags.value = { settings: true }
    mocks.routeName.value = 'Settings'
    app = createApp(ContestLayout)
    app.use(pinia)
    app.component('RouterView', { template: '<div>settings content</div>' })
    app.mount(host)
    for (let i = 0; i < 5; i++) await nextTick()

    expect(host.textContent).toContain('Hinina 设置')
    // 裸壳自带拖动区标题栏（decorations=false 下窗口移动全靠它）
    expect(host.querySelector('header[data-tauri-drag-region]')).toBeTruthy()
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

  it('无会话 + 设置路由但普通启动（无钥匙）：不进最小壳（登出过渡态在设置页上同样安全）', async () => {
    // 普通启动永远不进最小壳（第三轮评审）：即使会话缺失且正停在设置路由——
    // 例如已登录维护者经隐藏入口进设置页后登出的过渡态——也保持原外壳
    const pinia = createPinia()
    setActivePinia(pinia)
    useAuthStore().sessionResolved = true
    mocks.launchFlags.value = { settings: false }
    mocks.routeName.value = 'Settings'
    app = createApp(ContestLayout)
    app.use(pinia)
    app.component('RouterView', { template: '<div>settings content</div>' })
    app.mount(host)
    for (let i = 0; i < 5; i++) await nextTick()

    expect(host.textContent).not.toContain('Hinina 设置')
    expect(host.querySelector('header[data-tauri-drag-region]')).toBeNull()
    expect(host.querySelector('footer')).toBeTruthy()
  })

  it('无会话但路由非设置页（正常登出过渡态）：保持正常外壳，不闪最小壳', async () => {
    // 登出把 user 置空而 sessionResolved 仍为 true：若最小壳只判会话，比赛页
    // 外壳会在 replace(Login) 完成前闪成「Hinina 设置」并重挂子视图（评审抓出）
    const pinia = createPinia()
    setActivePinia(pinia)
    useAuthStore().sessionResolved = true
    mocks.launchFlags.value = { settings: false }
    mocks.routeName.value = 'ProblemSet'
    app = createApp(ContestLayout)
    app.use(pinia)
    app.component('RouterView', { template: '<div>contest content</div>' })
    app.mount(host)
    for (let i = 0; i < 5; i++) await nextTick()

    expect(host.textContent).not.toContain('Hinina 设置')
    expect(host.querySelector('header[data-tauri-drag-region]')).toBeNull()
    // 正常外壳的 TopBar / StatusBar 桩仍在
    expect(host.querySelector('footer')).toBeTruthy()
  })

  it('钥匙启动下在比赛子路由登出（钥匙真 + 非设置路由 + 无会话）：保持正常外壳', async () => {
    // 真实场景：--settings 启动 → 返回登录 → 登录进场 → 在比赛页登出。此刻
    // 钥匙为真、路由是比赛子路由、会话缺失 —— 若 bareShell 漏掉路由条件，
    // 外壳会在这里闪成最小壳。上面的「无钥匙 + 非设置路由」用例测的是钥匙
    // 条件，路由条件只有本用例锁得住（评审指出条件互相遮蔽）。
    const pinia = createPinia()
    setActivePinia(pinia)
    useAuthStore().sessionResolved = true
    mocks.launchFlags.value = { settings: true }
    mocks.routeName.value = 'ProblemSet'
    app = createApp(ContestLayout)
    app.use(pinia)
    app.component('RouterView', { template: '<div>contest content</div>' })
    app.mount(host)
    for (let i = 0; i < 5; i++) await nextTick()

    expect(host.textContent).not.toContain('Hinina 设置')
    expect(host.querySelector('header[data-tauri-drag-region]')).toBeNull()
    expect(host.querySelector('footer')).toBeTruthy()
  })

  it('评审四 P2-2：最小壳下保留的已结束简报不触发赛程跳转', async () => {
    // 钥匙 + 无会话 + 设置路由，且登出清理只清会话部分（clearSessionData 保留
    // 匿名简报）：若阶段 watcher 不看 bareShell，「已结束」简报会在 2 秒后把
    // 维护者从设置页踢回登录页（评审四 P2-2 复现路径）
    vi.useFakeTimers()
    const now = Math.floor(Date.now() / 1000)
    const pinia = createPinia()
    setActivePinia(pinia)
    mocks.launchFlags.value = { settings: true }
    mocks.routeName.value = 'Settings'
    const store = useContestStore()
    store.brief = { ...schedule, startTime: now - 7_200, endTime: now - 3_600 }
    app = createApp(ContestLayout)
    app.use(pinia)
    app.component('RouterView', { template: '<div>settings content</div>' })
    app.mount(host)
    for (let i = 0; i < 5; i++) await nextTick()

    expect(host.textContent).toContain('Hinina 设置')
    await vi.advanceTimersByTimeAsync(2_500)
    expect(mocks.replace).not.toHaveBeenCalled() // 已结束简报不得踢出最小壳
    expect(host.querySelector('header[data-tauri-drag-region]')).toBeTruthy()
    vi.useRealTimers()
  })

  it('评审四 P2-2：最小壳下保留的未开始简报不触发立即跳转', async () => {
    vi.useFakeTimers()
    const now = Math.floor(Date.now() / 1000)
    const pinia = createPinia()
    setActivePinia(pinia)
    mocks.launchFlags.value = { settings: true }
    mocks.routeName.value = 'Settings'
    const store = useContestStore()
    store.brief = { ...schedule, startTime: now + 3_600, endTime: now + 14_400 }
    app = createApp(ContestLayout)
    app.use(pinia)
    app.component('RouterView', { template: '<div>settings content</div>' })
    app.mount(host)
    for (let i = 0; i < 5; i++) await nextTick()

    expect(host.textContent).toContain('Hinina 设置')
    expect(mocks.replace).not.toHaveBeenCalled() // upcoming 的立即跳转被最小壳挡下
    vi.useRealTimers()
  })
})
