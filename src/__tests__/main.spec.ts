// @vitest-environment jsdom
// main.ts 组合根装配（M2 工作包 C，轻量）：四个装配副作用必须发生，
// 公告事件按 contestId 过滤（切比赛瞬间的在途事件不得刷新新比赛的公告）。
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({
  installSessionGuard: vi.fn(),
  installWorkspacePersistenceListener: vi.fn(),
  onAnnouncementsPublished: vi.fn<(cb: (p: { contestId: string }) => void) => Promise<() => void>>(
    async () => vi.fn() as unknown as () => void,
  ),
  createCloseGuard: vi.fn(() => ({ handleRequest: () => false })),
  onCloseRequested: vi.fn(async () => vi.fn()),
  closeWindow: vi.fn(),
  destroyWindow: vi.fn(),
  announcementRefresh: vi.fn(async () => {}),
}))

vi.mock('vue-router', () => ({
  useRoute: () => ({ matched: [], meta: {}, query: {}, params: {}, path: '/' }),
  RouterLink: { template: '<a><slot /></a>' },
  RouterView: { template: '<div />' },
}))
vi.mock('@/router', () => ({ default: { install: vi.fn() } }))
vi.mock('@/guards/sessionGuard', () => ({ installSessionGuard: mocks.installSessionGuard }))
vi.mock('@/stores/workspaceStore', () => ({
  useWorkspaceStore: () => ({ saveWorkspace: vi.fn(), syncPending: false, isDirty: false }),
  installWorkspacePersistenceListener: mocks.installWorkspacePersistenceListener,
}))
vi.mock('@/stores/announcementStore', () => ({
  useAnnouncementStore: () => ({ contestId: 'c1' as string | null, refresh: mocks.announcementRefresh }),
}))
vi.mock('@/bridge/announcement.bridge', () => ({
  onAnnouncementsPublished: mocks.onAnnouncementsPublished,
}))
vi.mock('@/utils/close-guard', () => ({ createCloseGuard: mocks.createCloseGuard }))
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    close: mocks.closeWindow,
    destroy: mocks.destroyWindow,
    onCloseRequested: mocks.onCloseRequested,
  }),
}))
vi.mock('@/styles/global.css', () => ({}))
vi.mock('@/styles/contest.css', () => ({}))
vi.mock('katex/dist/katex.min.css', () => ({}))

describe('main.ts 组合根装配', () => {
  /// 导入期装配事实的快照。vite.config 的全局 `clearMocks: true` 会在每个测试前
  /// 清空 mock 调用记录，而装配发生在 beforeAll 的导入期 —— 必须当场快照成普通值，
  /// 否则断言看到的永远是 0。
  const installed = {
    sessionGuard: 0,
    workspaceListener: 0,
    announcements: 0,
    closeGuard: 0,
    announcementHandler: null as ((p: { contestId: string }) => void) | null,
  }

  beforeAll(async () => {
    // DEV 分支会在 window 上 defineProperty('hininaPlugins')（不可配置），
    // 重复导入必然 "Cannot redefine property"（CI 上以未处理错误炸掉整个 run）。
    // 该分支是开发台便利设施、不属于被测装配面 —— stub 为 false 跳过。
    vi.stubEnv('DEV', false)
    // 挂载发生在导入期：容器必须先于 import 存在（Vue 对缺失容器静默不挂）
    document.body.innerHTML = '<div id="app"></div>'
    await import('@/main')
    installed.sessionGuard = mocks.installSessionGuard.mock.calls.length
    installed.workspaceListener =
      mocks.installWorkspacePersistenceListener.mock.calls.length
    installed.announcements = mocks.onAnnouncementsPublished.mock.calls.length
    installed.closeGuard = mocks.onCloseRequested.mock.calls.length
    installed.announcementHandler =
      mocks.onAnnouncementsPublished.mock.calls[0]?.[0] ?? null
  })

  // beforeEach 不重置 DOM：挂载只发生一次，重置会把被测事实清掉

  it('四个装配副作用全部注册', async () => {
    expect(installed.sessionGuard).toBe(1)
    expect(installed.workspaceListener).toBe(1)
    expect(installed.announcements).toBe(1)
    expect(installed.closeGuard).toBe(1) // 关窗握手必须挂上真实窗口
    expect(document.querySelector('#app')?.innerHTML).not.toBe('') // 应用应挂载到 #app
  })

  it('公告事件：当前比赛的公告触发刷新，其它比赛的在途事件被丢弃', async () => {
    const handler = installed.announcementHandler
    expect(handler).toBeTypeOf('function')

    handler!({ contestId: 'c1' })
    await Promise.resolve()
    expect(mocks.announcementRefresh).toHaveBeenCalledTimes(1)

    handler!({ contestId: 'old-contest' })
    await Promise.resolve()
    expect(mocks.announcementRefresh).toHaveBeenCalledTimes(1) // 切比赛瞬间的旧比赛事件不得刷新新比赛公告
  })
})
