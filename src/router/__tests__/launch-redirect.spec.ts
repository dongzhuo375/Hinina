// @vitest-environment jsdom
// --settings 管理方钥匙的**守卫行为**测试（区别于 index.spec.ts 的 meta 合并矩阵）：
// firstNavigation 只触发一次、仅落登录页时直落、IPC 失败按「无参数」退化。
// 「每次导航到 Login 都重定向」会把维护人员困死在设置页（返回登录失效）——
// 这类回归在真实 router 上锁定（沿 contest-access.spec 的 mock 模式）。
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useAuthStore } from '@/stores/authStore'
import { useContestStore } from '@/stores/contestStore'

vi.mock('vue-router', async (importOriginal) => {
  const actual = await importOriginal<typeof import('vue-router')>()
  return { ...actual, createWebHistory: actual.createMemoryHistory }
})
vi.mock('@/views/LoginView.vue', () => ({ default: { template: '<div />' } }))
vi.mock('@/views/ContestLayout.vue', () => ({ default: { template: '<div />' } }))
vi.mock('@/views/SettingsView.vue', () => ({ default: { template: '<div />' } }))
vi.mock('@/views/ProblemSetView.vue', () => ({ default: { template: '<div />' } }))

// 启动标志状态：getLaunchFlags 返回它，isKeyedSettings 同步读它（与真实实现同判据）
const launchFlags = vi.hoisted(() => ({ state: { settings: false, fail: false } }))
vi.mock('@/services/system.service', () => ({
  systemService: {
    getLaunchFlags: async () => {
      if (launchFlags.state.fail) throw new Error('invoke failed')
      return { settings: launchFlags.state.settings }
    },
    isKeyedSettings: (name?: string | symbol | null) =>
      launchFlags.state.settings && name === 'Settings',
  },
}))

beforeEach(() => {
  // firstNavigation 是 router 模块的模块级状态：resetModules 让每个用例拿到
  // 全新 router 实例（首次导航语义才可独立断言）
  vi.resetModules()
  launchFlags.state.settings = false
  launchFlags.state.fail = false
  setActivePinia(createPinia())
  // 会话已判定但不存在（--settings 的目标形态）；login 不受保护，守卫不拦
  const auth = useAuthStore()
  auth.sessionResolved = true
  auth.user = null
})

describe('--settings 初始直落（守卫行为）', () => {
  it('带参首次导航：登录页直落设置页', async () => {
    launchFlags.state.settings = true
    const { default: router } = await import('@/router')
    await router.push({ name: 'Login' })
    expect(router.currentRoute.value.name).toBe('Settings')
  })

  it('仅首次生效：再次前往登录页不被重定向（返回登录可用，不困死维护人员）', async () => {
    launchFlags.state.settings = true
    const { default: router } = await import('@/router')
    await router.push({ name: 'Login' })
    expect(router.currentRoute.value.name).toBe('Settings')
    await router.push({ name: 'Login' })
    expect(router.currentRoute.value.name).toBe('Login')
  })

  it('首次导航不是登录页时不直落（深链照常走会话门禁）', async () => {
    launchFlags.state.settings = true
    const { default: router } = await import('@/router')
    await router.push({ name: 'ProblemSet' })
    // 无会话：受保护路由按原守卫回登录页，而不是被钥匙带去设置页
    expect(router.currentRoute.value.name).toBe('Login')
  })

  it('flags IPC 失败按「无参数」退化：首航停留登录页，不炸守卫', async () => {
    launchFlags.state.fail = true
    const { default: router } = await import('@/router')
    await router.push({ name: 'Login' })
    expect(router.currentRoute.value.name).toBe('Login')
  })

  it('无参启动：不重定向（正常门禁形态）', async () => {
    launchFlags.state.settings = false
    const { default: router } = await import('@/router')
    await router.push({ name: 'Login' })
    expect(router.currentRoute.value.name).toBe('Login')
  })

  it('无钥匙 + 无会话直达设置页：回登录页（meta 继承保护，豁免仅钥匙模式）', async () => {
    // 评审 P2 路径一：无条件豁免会让无钥匙访客直达设置页
    launchFlags.state.settings = false
    const { default: router } = await import('@/router')
    await router.push({ name: 'Settings' })
    expect(router.currentRoute.value.name).toBe('Login')
  })

  it('评审九 P2-2：导航中途会话失效——钥匙豁免不被在途的比赛导航继承', async () => {
    // 时序：守卫批准 ProblemSet 后、路由确认前，401 把会话清掉。sessionGuard
    // 此刻读到的 currentRoute 仍是设置页，钥匙豁免会跳过跳转——落地后的
    // afterEach 不变量必须兜住（受保护路由 + 无会话 + 非设置页 → 回登录页）。
    launchFlags.state.settings = true
    const now = Math.floor(Date.now() / 1000)
    const store = useContestStore()
    store.brief = {
      id: '1', title: '竞赛', startTime: now - 3_600, endTime: now + 3_600,
      description: '', contestType: 0, status: 0, auth: 0,
      rankShowName: 'username', sealRank: false, sealRankTime: null,
      allowEndSubmit: false, oiRankScoreType: null,
    }
    const auth = useAuthStore()
    auth.user = { id: 'u1', username: 'maintainer', token: 't' } as never

    const { default: router } = await import('@/router')
    // 首航：钥匙直落设置页（豁免路径的起点）
    await router.push({ name: 'Login' })
    expect(router.currentRoute.value.name).toBe('Settings')

    // 守卫批准后、确认前清掉会话（beforeResolve 介于 beforeEach 与落地之间）
    const stop = router.beforeResolve(() => {
      useAuthStore().user = null
    })
    await router.push({ name: 'ProblemSet' })
    stop()

    // 落地不变量：不留在无会话的比赛页（afterEach 的 replace 是 fire-and-forget，
    // 等替换导航落地）
    await vi.waitFor(() => expect(router.currentRoute.value.name).toBe('Login'))
  })

  it('评审十 #1：钥匙设置页出发、赛前/赛后目标比赛页——回设置页而非登录页', async () => {
    // 否则落进被闸门锁定的比赛页 → 外壳 watcher 踢回登录页 → 草稿丢失且无回入口
    launchFlags.state.settings = true
    const now = Math.floor(Date.now() / 1000)
    const store = useContestStore()
    // 已结束的简报：从设置页点题目/榜单应被拦回设置页
    store.brief = {
      id: '1', title: '竞赛', startTime: now - 7_200, endTime: now - 3_600,
      description: '', contestType: 0, status: 0, auth: 0,
      rankShowName: 'username', sealRank: false, sealRankTime: null,
      allowEndSubmit: false, oiRankScoreType: null,
    }
    const auth = useAuthStore()
    auth.user = { id: 'u1', username: 'maintainer', token: 't' } as never

    const { default: router } = await import('@/router')
    await router.push({ name: 'Login' })
    expect(router.currentRoute.value.name).toBe('Settings')

    await router.push({ name: 'ProblemSet' })
    expect(router.currentRoute.value.name).toBe('Settings') // 回设置页，不是 Login
    // 评审十五 B：拦回时给一次可见反馈（NavigationFailure 本身静默）
    const { settingsGateNotice } = await import('@/utils/settings-gate-notice')
    expect(settingsGateNotice.value).toContain('比赛未进行')
  })

  it('评审十九 #2：守卫等待 loadBrief 期间会话失效——钥匙流程回设置页而非登录页', async () => {
    launchFlags.state.settings = true
    const auth = useAuthStore()
    auth.user = { id: 'u1', username: 'maintainer', token: 't' } as never
    const { default: router } = await import('@/router')
    await router.push({ name: 'Login' })
    expect(router.currentRoute.value.name).toBe('Settings')

    // brief 缺失 → 守卫 await loadBrief；期间会话被清（如一次 OJ 切换完成）
    const store = useContestStore()
    store.brief = null
    vi.spyOn(store, 'loadBrief').mockImplementation(async () => {
      auth.user = null
    })
    await router.push({ name: 'ProblemSet' })
    expect(router.currentRoute.value.name).toBe('Settings')
    // 评审二十六 P3：拦回时必须有可见反馈（与未登录分支一致）
    const { settingsGateNotice } = await import('@/utils/settings-gate-notice')
    expect(settingsGateNotice.value).toContain('会话已失效')
  })

  it('评审二十：钥匙设置页 + 无会话点赛场页面——留在设置页并提示，不去登录页', async () => {
    // 无会话（最小壳）时点/历史回到 ProblemSet：无条件 Login 会卸载设置视图、
    // 丢掉草稿——与「钥匙失败回设置页」的设计同向
    launchFlags.state.settings = true
    const { default: router } = await import('@/router')
    await router.push({ name: 'Login' })
    expect(router.currentRoute.value.name).toBe('Settings')

    await router.push({ name: 'ProblemSet' })
    expect(router.currentRoute.value.name).toBe('Settings')
    const { settingsGateNotice } = await import('@/utils/settings-gate-notice')
    expect(settingsGateNotice.value).toContain('未登录')
  })

  it('评审十九 #3：简报加载失败时提示说明「无法加载」，而不是「比赛未进行」', async () => {
    launchFlags.state.settings = true
    const auth = useAuthStore()
    auth.user = { id: 'u1', username: 'maintainer', token: 't' } as never
    const { default: router } = await import('@/router')
    await router.push({ name: 'Login' })

    const store = useContestStore()
    store.brief = null
    vi.spyOn(store, 'loadBrief').mockResolvedValue(undefined) // 失败：brief 仍为 null
    await router.push({ name: 'ProblemSet' })

    const { settingsGateNotice } = await import('@/utils/settings-gate-notice')
    expect(settingsGateNotice.value).toContain('未能加载比赛信息')
    expect(settingsGateNotice.value).not.toContain('比赛未进行')
  })

  it('钥匙模式 + 无会话非首航直达设置页：放行（豁免不限首次）', async () => {
    // 首航去别处后，--settings 进程里设置页仍可无会话直达（返回登录后想再修
    // 一次地址的场景）
    launchFlags.state.settings = true
    const { default: router } = await import('@/router')
    await router.push({ name: 'Login' })
    expect(router.currentRoute.value.name).toBe('Settings')
    await router.push({ name: 'Login' })
    await router.push({ name: 'Settings' })
    expect(router.currentRoute.value.name).toBe('Settings')
  })
})
