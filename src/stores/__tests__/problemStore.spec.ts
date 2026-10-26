import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

/// Service 层打桩：store 只依赖 service，测试不触达 IPC
const { problemService } = vi.hoisted(() => ({
  problemService: {
    getProblem: vi.fn(),
    getProblemLimits: vi.fn(),
    getUserProblemStatus: vi.fn(),
  },
}))
vi.mock('@/services/problem.service', () => ({ problemService }))

import { useProblemStore } from '@/stores/problemStore'
import type { Problem } from '@/types/problem'

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

// ── M2 工作包 C：openProblem / loadLimits 的失败语义（此前未覆盖）──

const problemOf = (id: string) => ({
  id, title: `题-${id}`, description: '', inputDescription: '', outputDescription: '',
  samples: [], timeLimit: 1000, memoryLimit: 256, languages: ['C++'],
})

describe('openProblem 失败语义', () => {
  it('失败重抛并写入 error，currentProblem 保持旧题', async () => {
    problemService.getProblem.mockRejectedValue(new Error('BOOM'))
    const store = useProblemStore()
    store.currentProblem = problemOf('old') as never

    await expect(store.openProblem(CONTEST_ID, 'A')).rejects.toThrow('BOOM')

    expect(store.error).toContain('BOOM')
    expect((store.currentProblem as { id: string }).id).toBe('old') // openProblem 失败不得清空旧题（视图配对校验依赖此语义）
    expect(store.isLoading).toBe(false)
  })

  it('成功时清空既有错误并写入新题', async () => {
    problemService.getProblem.mockResolvedValue(problemOf('p1'))
    const store = useProblemStore()
    store.error = '上次失败'

    await store.openProblem(CONTEST_ID, 'A')

    expect((store.currentProblem as { id: string }).id).toBe('p1')
    expect(store.error).toBeNull()
  })
})

describe('loadLimits 失败语义', () => {
  it('失败吞错（不重抛）且保留旧值', async () => {
    problemService.getProblemLimits.mockRejectedValue(new Error('net'))
    const store = useProblemStore()
    store.limits = { A: { timeLimitMs: 1000, memoryLimitKb: 256 } } as never

    await expect(store.loadLimits(CONTEST_ID, ['A'])).resolves.toBeUndefined()

    expect(store.limits.A).toBeDefined() // 旧值必须保留，视图显示占位而不是清空
    expect(store.isLimitsLoading).toBe(false)
  })

  it('空列表短路，不打请求', async () => {
    const store = useProblemStore()
    await store.loadLimits(CONTEST_ID, [])
    expect(problemService.getProblemLimits).not.toHaveBeenCalled()
  })
})

describe('openProblem 过期响应保护（评审 P1，store 层）', () => {
  interface Deferred<T> {
    promise: Promise<T>
    resolve: (v: T) => void
    reject: (e: unknown) => void
  }
  const deferred = <T,>(): Deferred<T> => {
    let resolve!: (v: T) => void
    let reject!: (e: unknown) => void
    const promise = new Promise<T>((res, rej) => { resolve = res; reject = rej })
    return { promise, resolve, reject }
  }

  it('迟到的旧题成功响应不得覆盖新题', async () => {
    const lateA = deferred<Problem>()
    problemService.getProblem.mockImplementationOnce(() => lateA.promise)
    problemService.getProblem.mockImplementationOnce(async () => problemOf('p-B') as never)

    const store = useProblemStore()
    const first = store.openProblem(CONTEST_ID, 'A')
    const second = store.openProblem(CONTEST_ID, 'B')
    await second
    lateA.resolve(problemOf('p-A') as never)
    await first

    // 路由已在 B：迟到的 A 响应不得把题面拉回旧题（提交配对挡得住错误提交，挡不住错误显示）
    expect((store.currentProblem as { id: string }).id).toBe('p-B')
    expect(store.isLoading).toBe(false)
    expect(store.error).toBeNull()
  })

  it('迟到的失败响应不占错误态、不重抛', async () => {
    const lateA = deferred<Problem>()
    problemService.getProblem.mockImplementationOnce(() => lateA.promise)
    problemService.getProblem.mockImplementationOnce(async () => problemOf('p-B') as never)

    const store = useProblemStore()
    const first = store.openProblem(CONTEST_ID, 'A')
    const second = store.openProblem(CONTEST_ID, 'B')
    await second
    lateA.reject(new Error('late boom'))
    await expect(first).resolves.toBeUndefined()

    expect(store.error).toBeNull() // 错误态由最新请求决定
    expect((store.currentProblem as { id: string }).id).toBe('p-B')
  })

  it('isLoading 只由最新一次请求收尾（旧请求 finally 不得提前关闭新加载态）', async () => {
    const lateA = deferred<Problem>()
    problemService.getProblem.mockImplementationOnce(() => lateA.promise)
    problemService.getProblem.mockImplementationOnce(async () => problemOf('p-B') as never)

    const store = useProblemStore()
    const first = store.openProblem(CONTEST_ID, 'A')
    const second = store.openProblem(CONTEST_ID, 'B')
    await second
    expect(store.isLoading).toBe(false) // B 已完成，加载态应结束

    lateA.resolve(problemOf('p-A') as never)
    await first
    expect(store.isLoading).toBe(false) // 过期的 finally 不得翻转加载态
  })
})

describe('会话代际（评审四轮 #1：旧用户的题目状态）', () => {
  it('迟到的 loadMyStatus 不得把旧会话 AC 状态写回新会话', async () => {
    const { clearDomainState } = await import('@/stores/session')
    const store = useProblemStore()
    let resolveStatus!: (v: Record<string, number>) => void
    problemService.getUserProblemStatus.mockImplementation(
      () => new Promise((resolve) => { resolveStatus = resolve }),
    )

    const pending = store.loadMyStatus(CONTEST_ID, PROBLEM_IDS)
    await vi.waitFor(() => expect(problemService.getUserProblemStatus).toHaveBeenCalled())

    clearDomainState()
    expect(store.myStatus).toEqual({})

    // 旧会话的「已 AC / 尝试过」不得显示给新用户
    resolveStatus({ '1061': 1, '1062': 2 })
    await pending
    expect(store.myStatus).toEqual({})
    expect(store.isStatusLoading).toBe(false)
  })
})
