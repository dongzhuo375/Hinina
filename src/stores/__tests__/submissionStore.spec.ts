import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import type { JudgementResult, SubmissionPage, SubmissionRecord } from '@/types/submission'

/// Service 层打桩：store 只依赖 service，测试不触达 IPC（逐层隔离约定）
const { submissionService, configService } = vi.hoisted(() => ({
  submissionService: {
    submitCode: vi.fn(),
    pollJudgement: vi.fn(),
    listContestSubmissions: vi.fn(),
    getSubmissionDetail: vi.fn(),
    getSubmissionCases: vi.fn(),
  },
  configService: {
    getPollSchedule: vi.fn(),
  },
}))
vi.mock('@/services/submission.service', () => ({ submissionService }))
vi.mock('@/services/config.service', () => ({ configService }))
/// 依赖链最终会 import Tauri API，一并打桩保持测试环境纯净
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

import { useSubmissionStore } from '@/stores/submissionStore'

const CONTEST_ID = '1'
/// 轮询节奏：2s 间隔（抖动 ≤500ms）、10s 总超时
const SCHEDULE = { intervalMs: 2_000, timeoutMs: 10_000 }

function makeResult(status: JudgementResult['status'], over: Partial<JudgementResult> = {}): JudgementResult {
  return { status, score: 0, timeMs: 15, memoryKb: 1024, ...over }
}

function makeRecord(over: Partial<SubmissionRecord> = {}): SubmissionRecord {
  return {
    submitId: '100',
    pid: '1001',
    displayPid: 'HOJ-1001',
    title: '两数之和',
    displayId: 'A',
    username: 'team01',
    submitTime: 1_700_000_000,
    status: 'Accepted',
    timeMs: 15,
    memoryKb: 1024,
    score: null,
    length: 256,
    language: 'C++',
    ...over,
  }
}

function makePage(records: SubmissionRecord[], total: number, current = 1): SubmissionPage {
  return { records, total, size: 20, current, pages: Math.max(1, Math.ceil(total / 20)) }
}

beforeEach(() => {
  setActivePinia(createPinia())
  vi.clearAllMocks()
  configService.getPollSchedule.mockResolvedValue(SCHEDULE)
  vi.spyOn(console, 'error').mockImplementation(() => {})
  vi.spyOn(console, 'warn').mockImplementation(() => {})
})

describe('submitCode + 评测轮询（P54：createPoller 收敛轮询）', () => {
  it('提交成功后记录条目并启动轮询，终态后自动停止', async () => {
    vi.useFakeTimers()
    try {
      submissionService.submitCode.mockResolvedValue('s1')
      submissionService.pollJudgement.mockResolvedValue(makeResult('Accepted'))
      const store = useSubmissionStore()

      await store.submitCode(CONTEST_ID, 'p1', 'cpp', 'int main(){}')
      expect(store.submissions).toHaveLength(1)
      expect(store.submissions[0].status).toBe('Pending')

      // 首次轮询在一个完整周期（2s±0.4s）之后
      await vi.advanceTimersByTimeAsync(2_600)
      expect(submissionService.pollJudgement).toHaveBeenCalledTimes(1)
      expect(store.submissions[0].status).toBe('Accepted')

      // 终态后停止：再推进多个周期也不发请求
      await vi.advanceTimersByTimeAsync(30_000)
      expect(submissionService.pollJudgement).toHaveBeenCalledTimes(1)
    } finally {
      vi.useRealTimers()
    }
  })

  it('评测结果回填耗时与内存（memory 不得缺失）', async () => {
    vi.useFakeTimers()
    try {
      submissionService.submitCode.mockResolvedValue('s1')
      submissionService.pollJudgement.mockResolvedValue(
        makeResult('WrongAnswer', { timeMs: 2010, memoryKb: 65_536 }),
      )
      const store = useSubmissionStore()

      await store.submitCode(CONTEST_ID, 'p1', 'cpp', 'code')
      await vi.advanceTimersByTimeAsync(2_600)

      expect(store.submissions[0].time).toBe(2010)
      expect(store.submissions[0].memory).toBe(65_536)
    } finally {
      vi.useRealTimers()
    }
  })

  it('非终态持续轮询，超过总超时后兜底停止', async () => {
    vi.useFakeTimers()
    try {
      submissionService.submitCode.mockResolvedValue('s1')
      submissionService.pollJudgement.mockResolvedValue(makeResult('Pending'))
      const store = useSubmissionStore()

      await store.submitCode(CONTEST_ID, 'p1', 'cpp', 'code')
      // 超时 10s：推进到超时后必然停止
      await vi.advanceTimersByTimeAsync(11_000)
      const callsAtTimeout = submissionService.pollJudgement.mock.calls.length
      expect(callsAtTimeout).toBeGreaterThanOrEqual(4)

      await vi.advanceTimersByTimeAsync(60_000)
      expect(submissionService.pollJudgement).toHaveBeenCalledTimes(callsAtTimeout)
      expect(console.warn).toHaveBeenCalled()
    } finally {
      vi.useRealTimers()
    }
  })

  it('瞬时失败不中断轮询；恢复成功后清除残留错误（P69）', async () => {
    vi.useFakeTimers()
    try {
      submissionService.submitCode.mockResolvedValue('s1')
      submissionService.pollJudgement
        .mockRejectedValueOnce(new Error('网络抖动'))
        .mockResolvedValue(makeResult('Accepted'))
      const store = useSubmissionStore()

      await store.submitCode(CONTEST_ID, 'p1', 'cpp', 'code')
      await vi.advanceTimersByTimeAsync(2_600)
      expect(store.error).toBe('网络抖动')

      await vi.advanceTimersByTimeAsync(2_600)
      // 成功路径必须清 error，否则失败红字永久残留（P69）
      expect(store.error).toBeNull()
      expect(store.submissions[0].status).toBe('Accepted')
    } finally {
      vi.useRealTimers()
    }
  })

  it('stopAllPolling 回收全部轮询器（登出后不再发请求）', async () => {
    vi.useFakeTimers()
    try {
      submissionService.submitCode.mockResolvedValueOnce('s1').mockResolvedValueOnce('s2')
      submissionService.pollJudgement.mockResolvedValue(makeResult('Pending'))
      const store = useSubmissionStore()

      await store.submitCode(CONTEST_ID, 'p1', 'cpp', 'code1')
      await store.submitCode(CONTEST_ID, 'p2', 'cpp', 'code2')
      store.stopAllPolling()

      await vi.advanceTimersByTimeAsync(60_000)
      expect(submissionService.pollJudgement).not.toHaveBeenCalled()
    } finally {
      vi.useRealTimers()
    }
  })

  it('重复 startPolling 幂等：同一提交只有一个轮询器', async () => {
    vi.useFakeTimers()
    try {
      submissionService.submitCode.mockResolvedValue('s1')
      submissionService.pollJudgement.mockResolvedValue(makeResult('Pending'))
      const store = useSubmissionStore()

      await store.submitCode(CONTEST_ID, 'p1', 'cpp', 'code')
      await store.startPolling('s1')
      await store.startPolling('s1')

      await vi.advanceTimersByTimeAsync(2_600)
      // 若存在多个轮询器，同一周期会发出多次请求
      expect(submissionService.pollJudgement).toHaveBeenCalledTimes(1)
      store.stopAllPolling()
    } finally {
      vi.useRealTimers()
    }
  })
})

describe('提交历史（评测页数据源）', () => {
  it('fetchHistory 透传分页与筛选参数并写入状态', async () => {
    submissionService.listContestSubmissions.mockResolvedValue(makePage([makeRecord()], 42, 2))
    const store = useSubmissionStore()
    store.history.problemFilter = 'B'
    store.history.statusFilter = 5

    await store.fetchHistory(CONTEST_ID, 2)

    expect(submissionService.listContestSubmissions).toHaveBeenCalledWith({
      contestId: CONTEST_ID,
      currentPage: 2,
      limit: 20,
      problemDisplayId: 'B',
      status: 5,
    })
    expect(store.history.records).toHaveLength(1)
    expect(store.history.total).toBe(42)
    expect(store.history.current).toBe(2)
    expect(store.history.isLoading).toBe(false)
  })

  it('fetchHistory 失败写入 history.error 并抛出', async () => {
    submissionService.listContestSubmissions.mockRejectedValue(new Error('服务端异常'))
    const store = useSubmissionStore()

    await expect(store.fetchHistory(CONTEST_ID)).rejects.toThrow('服务端异常')
    expect(store.history.error).toBe('服务端异常')
    expect(store.history.isLoading).toBe(false)
  })

  it('切换筛选回到第 1 页', async () => {
    submissionService.listContestSubmissions.mockResolvedValue(makePage([], 0))
    const store = useSubmissionStore()
    store.history.current = 3

    await store.setHistoryProblemFilter(CONTEST_ID, 'C')

    expect(store.history.problemFilter).toBe('C')
    expect(submissionService.listContestSubmissions).toHaveBeenCalledWith(
      expect.objectContaining({ currentPage: 1, problemDisplayId: 'C' }),
    )
  })

  it('setHistoryPage 越界不发请求', async () => {
    submissionService.listContestSubmissions.mockResolvedValue(makePage([], 40))
    const store = useSubmissionStore()
    await store.fetchHistory(CONTEST_ID)
    const calls = submissionService.listContestSubmissions.mock.calls.length

    await store.setHistoryPage(CONTEST_ID, 99)
    await store.setHistoryPage(CONTEST_ID, 0)
    expect(submissionService.listContestSubmissions).toHaveBeenCalledTimes(calls)
  })

  it('fetchProblemSummary 只取 1 条并返回 latest + total；无提交时 latest 为 null', async () => {
    submissionService.listContestSubmissions.mockResolvedValue(makePage([makeRecord()], 12))
    const store = useSubmissionStore()

    const summary = await store.fetchProblemSummary(CONTEST_ID, 'A')

    expect(submissionService.listContestSubmissions).toHaveBeenCalledWith({
      contestId: CONTEST_ID,
      currentPage: 1,
      limit: 1,
      problemDisplayId: 'A',
      status: null,
    })
    expect(summary.latest?.submitId).toBe('100')
    expect(summary.total).toBe(12)

    submissionService.listContestSubmissions.mockResolvedValue(makePage([], 0))
    const empty = await store.fetchProblemSummary(CONTEST_ID, 'B')
    expect(empty.latest).toBeNull()
    expect(empty.total).toBe(0)
  })

  it('fetchProblemSummary 不污染评测页 history 状态', async () => {
    submissionService.listContestSubmissions.mockResolvedValue(makePage([makeRecord()], 12))
    const store = useSubmissionStore()

    await store.fetchProblemSummary(CONTEST_ID, 'A')

    expect(store.history.records).toEqual([])
    expect(store.history.total).toBe(0)
  })
})

describe('latestLocalFor getter', () => {
  it('返回指定题目本会话最新一条提交', async () => {
    submissionService.submitCode.mockResolvedValueOnce('s1').mockResolvedValueOnce('s2')
    submissionService.pollJudgement.mockResolvedValue(makeResult('Pending'))
    const store = useSubmissionStore()

    await store.submitCode(CONTEST_ID, 'p1', 'cpp', 'code1')
    await store.submitCode(CONTEST_ID, 'p1', 'cpp', 'code2')
    store.stopAllPolling()

    expect(store.latestLocalFor('p1')?.id).toBe('s2')
    expect(store.latestLocalFor('p2')).toBeNull()
  })
})
