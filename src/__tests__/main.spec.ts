// @vitest-environment jsdom
// main.ts 组合根装配（M2 工作包 C，轻量）：四个装配副作用必须发生，
// 公告事件按 contestId 过滤（切比赛瞬间的在途事件不得刷新新比赛的公告）。
import { beforeEach, describe, expect, it, vi } from 'vitest'

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
  beforeEach(() => {
    vi.clearAllMocks()
    // main.ts 模块级装配只执行一次；逐用例重置模块注册表保证重新装配
    vi.resetModules()
    document.body.innerHTML = '<div id="app"></div>'
  })

  it('四个装配副作用全部注册', async () => {
    await import('@/main')

    expect(mocks.installSessionGuard).toHaveBeenCalledTimes(1)
    expect(mocks.installWorkspacePersistenceListener).toHaveBeenCalledTimes(1)
    expect(mocks.onAnnouncementsPublished).toHaveBeenCalledTimes(1)
    expect(mocks.onCloseRequested).toHaveBeenCalledTimes(1) // 关窗握手必须挂上真实窗口
    expect(document.querySelector('#app')?.innerHTML).not.toBe('') // 应用应挂载到 #app
  })

  it('公告事件：当前比赛的公告触发刷新，其它比赛的在途事件被丢弃', async () => {
    await import('@/main')
    const handler = mocks.onAnnouncementsPublished.mock.calls[0]![0]

    handler({ contestId: 'c1' })
    await Promise.resolve()
    expect(mocks.announcementRefresh).toHaveBeenCalledTimes(1)

    handler({ contestId: 'old-contest' })
    await Promise.resolve()
    expect(mocks.announcementRefresh).toHaveBeenCalledTimes(1) // 切比赛瞬间的旧比赛事件不得刷新新比赛公告
  })
})
