import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

/// Service 层打桩：store 只依赖 service，测试不触达 IPC
const { problemService } = vi.hoisted(() => ({
  problemService: {
    getProblem: vi.fn(),
    listProblems: vi.fn(),
    getProblemLimits: vi.fn(),
    getUserProblemStatus: vi.fn(),
  },
}))
vi.mock('@/services/problem.service', () => ({ problemService }))

import { useProblemStore } from '@/stores/problemStore'

const CONTEST_ID = '1011'
const PROBLEM_IDS = ['1061', '1062']

beforeEach(() => {
  setActivePinia(createPinia())
  vi.spyOn(console, 'error').mockImplementation(() => {})
  vi.clearAllMocks()
})

describe('myStatus 增量失效 — 不再按轮询周期整表重拉', () => {
  it('初始为「已过期」：首屏必须拉一次', () => {
    expect(useProblemStore().myStatusStale).toBe(true)
  })

  it('loadMyStatus 成功后清除过期标记', async () => {
    problemService.getUserProblemStatus.mockResolvedValue({ '1061': 1 })
    const store = useProblemStore()

    await store.loadMyStatus(CONTEST_ID, PROBLEM_IDS)

    expect(store.myStatusStale).toBe(false)
    expect(store.myStatus).toEqual({ '1061': 1 })
  })

  it('loadMyStatus 失败时保留过期标记（下一周期重试）', async () => {
    problemService.getUserProblemStatus.mockRejectedValue(new Error('网络异常'))
    const store = useProblemStore()
    store.myStatusStale = false

    await store.loadMyStatus(CONTEST_ID, PROBLEM_IDS)

    expect(store.myStatusStale).toBe(true)
    expect(store.isStatusLoading).toBe(false)
  })

  it('invalidateMyStatus 只置位、不发请求', () => {
    const store = useProblemStore()
    store.myStatusStale = false

    store.invalidateMyStatus()

    expect(store.myStatusStale).toBe(true)
    expect(problemService.getUserProblemStatus).not.toHaveBeenCalled()
  })

  it('空题目列表短路：不发请求也不清过期标记', async () => {
    const store = useProblemStore()

    await store.loadMyStatus(CONTEST_ID, [])

    expect(problemService.getUserProblemStatus).not.toHaveBeenCalled()
    expect(store.myStatusStale).toBe(true)
  })
})

describe('myStatus 数据归属 — 比赛切换必须重拉', () => {
  it('无数据时任何比赛都需要重拉', () => {
    const store = useProblemStore()

    expect(store.myStatusNeedsReloadFor(CONTEST_ID)).toBe(true)
  })

  it('加载成功后记录归属比赛，同比赛不再重拉', async () => {
    problemService.getUserProblemStatus.mockResolvedValue({ '1061': 1 })
    const store = useProblemStore()

    await store.loadMyStatus(CONTEST_ID, PROBLEM_IDS)

    expect(store.myStatusContestId).toBe(CONTEST_ID)
    expect(store.myStatusNeedsReloadFor(CONTEST_ID)).toBe(false)
  })

  it('归属比赛不同时必须重拉（状态表以 pid 为键，同题可能出现在多场比赛）', async () => {
    problemService.getUserProblemStatus.mockResolvedValue({ '1061': 1 })
    const store = useProblemStore()
    await store.loadMyStatus(CONTEST_ID, PROBLEM_IDS)

    expect(store.myStatusNeedsReloadFor('1012')).toBe(true)
  })

  it('过期标记与归属判据取并集', async () => {
    problemService.getUserProblemStatus.mockResolvedValue({})
    const store = useProblemStore()
    await store.loadMyStatus(CONTEST_ID, PROBLEM_IDS)
    expect(store.myStatusNeedsReloadFor(CONTEST_ID)).toBe(false)

    store.invalidateMyStatus()

    expect(store.myStatusNeedsReloadFor(CONTEST_ID)).toBe(true)
  })

  it('拉取失败不改变归属（表里仍是上次成功那场比赛的数据）', async () => {
    problemService.getUserProblemStatus.mockResolvedValueOnce({ '1061': 1 })
    const store = useProblemStore()
    await store.loadMyStatus(CONTEST_ID, PROBLEM_IDS)

    problemService.getUserProblemStatus.mockRejectedValueOnce(new Error('网络异常'))
    await store.loadMyStatus('1012', PROBLEM_IDS)

    expect(store.myStatusContestId).toBe(CONTEST_ID)
    expect(store.myStatusStale).toBe(true)
    // 归属仍是旧比赛 + 已过期 → 两个判据都要求重拉
    expect(store.myStatusNeedsReloadFor('1012')).toBe(true)
    expect(store.myStatusNeedsReloadFor(CONTEST_ID)).toBe(true)
  })
})
