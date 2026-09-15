import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

/// Service 层打桩：store 只依赖 service，测试不触达 IPC
const { contestService } = vi.hoisted(() => ({
  contestService: {
    loadConfiguredContest: vi.fn(),
    loadContestBrief: vi.fn(),
  },
}))
vi.mock('@/services/contest.service', () => ({ contestService }))
/// 依赖链最终会 import Tauri API，一并打桩保持测试环境纯净
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

import { useContestStore } from '@/stores/contestStore'
import type { Contest } from '@/types/contest'

const contest: Contest = {
  id: '1',
  title: '2024 校赛模拟赛',
  startTime: 1_000,
  endTime: 2_000,
  description: '# brief',
  contestType: 0,
  status: 0,
  auth: 0,
  rankShowName: 'username',
  sealRank: false,
  sealRankTime: null,
  allowEndSubmit: false,
}

const problems = [
  {
    id: 1,
    displayId: 'A',
    cid: 1,
    problemId: '1001',
    displayTitle: '两数之和',
    ac: 210,
    total: 298,
    color: '#FF0000',
  },
]

beforeEach(() => {
  setActivePinia(createPinia())
  vi.spyOn(console, 'error').mockImplementation(() => {})
})

describe('loadContest', () => {
  it('成功时填充比赛与题目列表', async () => {
    contestService.loadConfiguredContest.mockResolvedValue({ contest, problems })
    const store = useContestStore()

    await store.loadContest()

    expect(store.contest).toEqual(contest)
    expect(store.problems).toHaveLength(1)
    expect(store.isLoading).toBe(false)
    expect(store.error).toBeNull()
  })

  it('契约：并发调用共享同一个请求（外壳与视图同时挂载不得重复打服务端）', async () => {
    let resolveFn: (value: unknown) => void = () => {}
    contestService.loadConfiguredContest.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveFn = resolve
        }),
    )
    const store = useContestStore()

    const first = store.loadContest()
    const second = store.loadContest()
    expect(contestService.loadConfiguredContest).toHaveBeenCalledTimes(1)

    resolveFn({ contest, problems })
    await Promise.all([first, second])
    expect(store.contest).toEqual(contest)
  })

  it('失败时写入原因、抛出，并允许后续重试', async () => {
    contestService.loadConfiguredContest.mockRejectedValueOnce(new Error('比赛不存在'))
    const store = useContestStore()

    await expect(store.loadContest()).rejects.toThrow('比赛不存在')
    expect(store.error).toBe('比赛不存在')
    expect(store.isLoading).toBe(false)

    // in-flight 标记必须已清理，否则失败后再也发不出请求
    contestService.loadConfiguredContest.mockResolvedValueOnce({ contest, problems })
    await store.loadContest()
    expect(contestService.loadConfiguredContest).toHaveBeenCalledTimes(2)
    expect(store.contest).toEqual(contest)
  })

  it('并发调用遇到失败时，两个调用方都收到 rejection', async () => {
    let rejectFn: (reason: unknown) => void = () => {}
    contestService.loadConfiguredContest.mockImplementation(
      () =>
        new Promise((_resolve, reject) => {
          rejectFn = reject
        }),
    )
    const store = useContestStore()

    const first = store.loadContest()
    const second = store.loadContest()
    rejectFn(new Error('网络异常'))

    await expect(first).rejects.toThrow('网络异常')
    await expect(second).rejects.toThrow('网络异常')
    expect(contestService.loadConfiguredContest).toHaveBeenCalledTimes(1)
  })
})

describe('loadBrief — 登录页匿名比赛简报', () => {
  it('成功时写入简报、基址与连接状态', async () => {
    contestService.loadContestBrief.mockResolvedValue({
      status: 'ok',
      contest,
      baseUrl: 'https://hoj.example.com',
    })
    const store = useContestStore()

    await store.loadBrief()

    expect(store.brief).toEqual(contest)
    expect(store.briefBaseUrl).toBe('https://hoj.example.com')
    expect(store.briefState).toBe('connected')
    expect(store.briefError).toBeNull()
  })

  it('未配置比赛 ID 时标记 unconfigured 而不是 failed', async () => {
    contestService.loadContestBrief.mockResolvedValue({
      status: 'unconfigured',
      baseUrl: 'https://hoj.example.com',
    })
    const store = useContestStore()

    await store.loadBrief()

    expect(store.briefState).toBe('unconfigured')
    expect(store.brief).toBeNull()
  })

  it('请求失败时记录原因且不抛出（登录页不应因简报失败而中断）', async () => {
    contestService.loadContestBrief.mockRejectedValue(new Error('服务器连接失败'))
    const store = useContestStore()

    await expect(store.loadBrief()).resolves.toBeUndefined()
    expect(store.briefState).toBe('failed')
    expect(store.briefError).toBe('服务器连接失败')
  })
})

describe('clearSessionData — 登出清理', () => {
  it('清空会话相关状态，但保留匿名简报（切换账号时右侧氛围区不应空白）', async () => {
    contestService.loadContestBrief.mockResolvedValue({
      status: 'ok',
      contest,
      baseUrl: 'https://hoj.example.com',
    })
    const store = useContestStore()
    await store.loadBrief()

    contestService.loadConfiguredContest.mockResolvedValue({ contest, problems })
    await store.loadContest()
    store.error = '某个错误'

    store.clearSessionData()

    expect(store.contest).toBeNull()
    expect(store.problems).toEqual([])
    expect(store.error).toBeNull()
    expect(store.isLoading).toBe(false)
    // 匿名简报与登录态无关
    expect(store.brief).toEqual(contest)
    expect(store.briefState).toBe('connected')
  })
})
