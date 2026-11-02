// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, nextTick } from 'vue'
import type { App } from 'vue'
import { createPinia, setActivePinia } from 'pinia'
import { useContestStore } from '@/stores/contestStore'
import { useAuthStore } from '@/stores/authStore'
import type { Contest } from '@/types/contest'
import ContestLayout from '@/views/ContestLayout.vue'

/// jsdom 的 `inert` 反射怪癖：`el.inert = false` 会留下 `inert="false"` 属性
///（按 HTML 规范布尔属性「存在即生效」，真实浏览器里 Vue 对 false 是移除属性）。
/// 断言一律经此判读：属性缺失或值为 "false" 都视为未锁定。
function contentInert(host: HTMLElement): boolean {
  const el = host.querySelector('[inert]')
  return el !== null && el.getAttribute('inert') !== 'false'
}

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
  systemService: {
    launchFlags: mocks.launchFlags,
    // 与真实实现同判据（读取快照 + 路由名）
    isKeyedSettings: (name?: string | symbol | null) =>
      mocks.launchFlags.value.settings && name === 'Settings',
  },
}))
vi.mock('@/stores/announcementStore', () => ({ useAnnouncementStore: () => mocks }))
vi.mock('@/services/contest.service', () => ({ contestService: { loadConfiguredContest: mocks.loadConfiguredContest } }))
vi.mock('@/components/layout/TopBar.vue', () => ({ default: { template: '<header />' } }))
vi.mock('@/components/layout/WindowControls.vue', () => ({ default: { template: '<div data-test="wc" />' } }))
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
    // 评审十 #2：极简壳也有窗口控件（最小化/关闭），不只 Alt+F4
    expect(host.querySelector('[data-test="wc"]')).toBeTruthy()
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

  it('评审五：钥匙 + 会话有效 + 已结束简报 + 设置路由——不跳转（完整外壳保持）', async () => {
    // P2-2 的有会话分支：bareShell 为 false（正常外壳），但赛程闸门的钥匙短路
    // 看 keyedSettings —— 已结束简报不得把维护者从设置页踢回登录页
    vi.useFakeTimers()
    const now = Math.floor(Date.now() / 1000)
    const pinia = createPinia()
    setActivePinia(pinia)
    mocks.launchFlags.value = { settings: true }
    mocks.routeName.value = 'Settings'
    const auth = useAuthStore()
    auth.sessionResolved = true
    auth.user = { id: 'u1', username: 'maintainer', token: 't' } as never
    const store = useContestStore()
    store.brief = { ...schedule, startTime: now - 7_200, endTime: now - 3_600 }
    app = createApp(ContestLayout)
    app.use(pinia)
    app.component('RouterView', { template: '<div>settings content</div>' })
    app.mount(host)
    for (let i = 0; i < 5; i++) await nextTick()

    // 完整外壳（有会话）：TopBar/StatusBar 桩在，无最小壳
    expect(host.querySelector('header[data-tauri-drag-region]')).toBeNull()
    expect(host.querySelector('footer')).toBeTruthy()
    // 评审六：设置页不被赛程闸门锁死——无结束遮罩、内容区无 inert
    expect(host.querySelector('[role="alert"]')).toBeNull()
    expect(contentInert(host)).toBe(false)
    await vi.advanceTimersByTimeAsync(2_500)
    expect(mocks.replace).not.toHaveBeenCalled()
    vi.useRealTimers()
  })

  it('评审五反向：同条件但在比赛子路由——赛程闸门照常生效', async () => {
    // 短路只属于设置页：离开设置页回到比赛子路由后，已结束赛程照常退出
    //（防止滞留在过期赛场看旧数据——闸门的原始职责）
    vi.useFakeTimers()
    const now = Math.floor(Date.now() / 1000)
    const pinia = createPinia()
    setActivePinia(pinia)
    mocks.launchFlags.value = { settings: true }
    mocks.routeName.value = 'ProblemSet'
    const auth = useAuthStore()
    auth.sessionResolved = true
    auth.user = { id: 'u1', username: 'maintainer', token: 't' } as never
    const store = useContestStore()
    store.brief = { ...schedule, startTime: now - 7_200, endTime: now - 3_600 }
    app = createApp(ContestLayout)
    app.use(pinia)
    app.component('RouterView', { template: '<div>contest content</div>' })
    app.mount(host)
    for (let i = 0; i < 5; i++) await nextTick()

    await vi.advanceTimersByTimeAsync(2_500)
    expect(mocks.replace).toHaveBeenCalledWith({ name: 'Login' })
    // 反向：比赛子路由的赛程闸门模板形态照旧（遮罩 + inert）
    expect(host.querySelector('[role="alert"]')).toBeTruthy()
    expect(contentInert(host)).toBe(true)
    vi.useRealTimers()
  })

  it('评审六：钥匙 + 会话有效 + 未开始简报 + 设置路由——内容区无 inert', async () => {
    // upcoming 无遮罩但裸 phase 会给内容区加 inert——设置页同样必须可用
    vi.useFakeTimers()
    const now = Math.floor(Date.now() / 1000)
    const pinia = createPinia()
    setActivePinia(pinia)
    mocks.launchFlags.value = { settings: true }
    mocks.routeName.value = 'Settings'
    const auth = useAuthStore()
    auth.sessionResolved = true
    auth.user = { id: 'u1', username: 'maintainer', token: 't' } as never
    const store = useContestStore()
    store.brief = { ...schedule, startTime: now + 3_600, endTime: now + 14_400 }
    app = createApp(ContestLayout)
    app.use(pinia)
    app.component('RouterView', { template: '<div>settings content</div>' })
    app.mount(host)
    for (let i = 0; i < 5; i++) await nextTick()

    expect(contentInert(host)).toBe(false)
    expect(mocks.replace).not.toHaveBeenCalled()
    vi.useRealTimers()
  })

  it('评审八 P2-1：钥匙 + 有效会话 + 进行中比赛进设置页——自然结束停轮询但不跳转', async () => {
    // bootstrap 启动公告轮询后，本地 endTime 到期（服务端 status 仍未变）：
    // phase 进入 ended，watcher 的钥匙豁免只免「跳转」——ended 停轮询的清理
    // 必须照常执行，否则公告请求会一直打（评审八 P2-1 复现路径）
    vi.useFakeTimers()
    vi.setSystemTime(900 * 1_000)
    const pinia = createPinia()
    setActivePinia(pinia)
    mocks.launchFlags.value = { settings: true }
    mocks.routeName.value = 'Settings'
    const auth = useAuthStore()
    auth.sessionResolved = true
    auth.user = { id: 'u1', username: 'maintainer', token: 't' } as never
    const store = useContestStore()
    store.contest = { ...schedule } // startTime=900 / endTime=1002：此刻进行中
    app = createApp(ContestLayout)
    app.use(pinia)
    app.component('RouterView', { template: '<div>settings content</div>' })
    app.mount(host)
    for (let i = 0; i < 5; i++) await nextTick()

    // bootstrap 启动了公告轮询
    expect(mocks.startLive).toHaveBeenCalledTimes(1)

    // 本地 endTime 到期（服务端 status 未变）：ended → 停轮询 + 不跳转
    //（赛程窗 900s–1002s：快进 120s 越过 endTime）
    await vi.advanceTimersByTimeAsync(120_000)
    expect(mocks.stopLive).toHaveBeenCalled()
    expect(mocks.replace).not.toHaveBeenCalled()
    // 仍停在设置页（完整外壳）
    expect(host.querySelector('[data-tauri-drag-region]')).toBeNull()
    expect(host.querySelector('footer')).toBeTruthy()
    vi.useRealTimers()
  })

  it('评审十九 #5：赛前打开的外壳，开赛后补启公告轮询（无需重挂）', async () => {
    vi.useFakeTimers()
    vi.setSystemTime(890 * 1_000) // 赛程 900–1002：此刻 upcoming
    const pinia = createPinia()
    setActivePinia(pinia)
    mocks.launchFlags.value = { settings: true }
    mocks.routeName.value = 'Settings'
    const auth = useAuthStore()
    auth.sessionResolved = true
    auth.user = { id: 'u1', username: 'maintainer', token: 't' } as never
    const store = useContestStore()
    store.contest = { ...schedule }
    store.brief = { ...schedule }
    app = createApp(ContestLayout)
    app.use(pinia)
    app.component('RouterView', { template: '<div>settings content</div>' })
    app.mount(host)
    for (let i = 0; i < 5; i++) await nextTick()
    expect(mocks.startLive).not.toHaveBeenCalled() // 赛前不起轮询

    // 越过 startTime（900）：阶段跃迁到 running → 补启
    await vi.advanceTimersByTimeAsync(12_000)
    expect(mocks.startLive).toHaveBeenCalledWith('1', expect.any(Function))
    vi.useRealTimers()
  })

  it('评审二十九 #4：同一比赛 running→ended→running 后轮询重启', async () => {
    // 必须完整走 running 起步（liveFor=id）→ ended（stopLive 并清 liveFor）→
    // running（第二次 startLive）；直接从 ended 挂载时 liveFor 本就是 null，
    // 测不出「停止时未清 liveFor」这个缺陷（评审三十 #2）
    vi.useFakeTimers()
    const now = Math.floor(Date.now() / 1000)
    const pinia = createPinia()
    setActivePinia(pinia)
    mocks.launchFlags.value = { settings: true }
    mocks.routeName.value = 'Settings'
    const store = useContestStore()
    store.contest = { ...schedule, startTime: now - 3_600, endTime: now + 3_600 } // 进行中
    app = createApp(ContestLayout)
    app.use(pinia)
    app.component('RouterView', { template: '<div>settings content</div>' })
    app.mount(host)
    for (let i = 0; i < 5; i++) await nextTick()
    expect(mocks.startLive).toHaveBeenCalledTimes(1) // liveFor=id

    // 结束：停止轮询
    store.contest = { ...schedule, startTime: now - 7_200, endTime: now - 3_600 }
    await vi.advanceTimersByTimeAsync(1_500)
    expect(mocks.stopLive).toHaveBeenCalled()

    // 组织者延长赛程：同一比赛回到 running → 轮询必须重启
    store.contest = { ...schedule, startTime: now - 7_200, endTime: now + 3_600 }
    await vi.advanceTimersByTimeAsync(1_500)
    expect(mocks.startLive).toHaveBeenCalledTimes(2)
    vi.useRealTimers()
  })

  it('评审三十三 #5：钥匙设置页 running→upcoming 也停轮询并清 liveFor，回到 running 重启', async () => {
    // 组织者把进行中比赛的开始时间改到未来：不跳转是对的，但轮询器自身只查
    // 页面隐藏与 contest.status，不会因阶段回退自停——必须由外壳清理
    vi.useFakeTimers()
    const now = Math.floor(Date.now() / 1000)
    const pinia = createPinia()
    setActivePinia(pinia)
    mocks.launchFlags.value = { settings: true }
    mocks.routeName.value = 'Settings'
    const store = useContestStore()
    store.contest = { ...schedule, startTime: now - 3_600, endTime: now + 3_600 } // 进行中
    app = createApp(ContestLayout)
    app.use(pinia)
    app.component('RouterView', { template: '<div>settings content</div>' })
    app.mount(host)
    for (let i = 0; i < 5; i++) await nextTick()
    expect(mocks.startLive).toHaveBeenCalledTimes(1) // liveFor=id

    // running → upcoming：停轮询、清 liveFor、不跳转
    store.contest = { ...schedule, startTime: now + 3_600, endTime: now + 7_200 }
    await vi.advanceTimersByTimeAsync(1_500)
    expect(mocks.stopLive).toHaveBeenCalled()
    expect(mocks.replace).not.toHaveBeenCalled()

    // upcoming → running：轮询必须重启（证明 liveFor 已被清空）
    store.contest = { ...schedule, startTime: now - 3_600, endTime: now + 3_600 }
    await vi.advanceTimersByTimeAsync(1_500)
    expect(mocks.startLive).toHaveBeenCalledTimes(2)
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

  it('评审六：最小壳↔完整外壳翻转不重挂内容区（未保存草稿得以保留）', async () => {
    // 顶栏/状态条按形态切换，但 router-view 两形态共享同一实例：会话失效翻转
    // bareShell 时，设置视图的 DOM 节点必须是同一个——重挂会丢未保存草稿
    const pinia = createPinia()
    setActivePinia(pinia)
    mocks.launchFlags.value = { settings: true }
    mocks.routeName.value = 'Settings'
    const auth = useAuthStore()
    auth.sessionResolved = true
    auth.user = { id: 'u1', username: 'maintainer', token: 't' } as never
    app = createApp(ContestLayout)
    app.use(pinia)
    app.component('RouterView', { template: '<div data-test="rv">settings content</div>' })
    app.mount(host)
    for (let i = 0; i < 5; i++) await nextTick()
    const before = host.querySelector('[data-test="rv"]')
    expect(before).toBeTruthy()

    // 会话失效（user 置空）触发 bareShell 翻转：chrome 换、内容区不换
    auth.user = null
    for (let i = 0; i < 5; i++) await nextTick()

    expect(host.textContent).toContain('Hinina 设置') // 最小壳 chrome 已接管
    expect(host.querySelector('[data-test="rv"]')).toBe(before) // 同一 DOM 节点
  })
})
