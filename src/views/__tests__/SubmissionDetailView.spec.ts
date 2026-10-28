// @vitest-environment jsdom
import { nextTick, reactive } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mountView, settleView } from '@/test-support/view-mount'
import type { Mounted } from '@/test-support/view-mount'
import type { JudgementStatus, SubmissionCases, SubmissionDetail } from '@/types/submission'

interface Deferred<T> {
  promise: Promise<T>
  resolve: (value: T) => void
  reject: (reason: unknown) => void
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<T>((res, rej) => { resolve = res; reject = rej })
  return { promise, resolve, reject }
}

const route = reactive({ params: { submitId: 'A' } as Record<string, string> })
const mocks = vi.hoisted(() => ({
  getSubmissionDetail: vi.fn(),
  getSubmissionCases: vi.fn(),
  getPollSchedule: vi.fn(),
  whenLoaded: vi.fn(),
}))

vi.mock('vue-router', () => ({
  useRoute: () => route,
  useRouter: () => ({ push: vi.fn() }),
}))
vi.mock('@/services/submission.service', () => ({
  submissionService: {
    getSubmissionDetail: mocks.getSubmissionDetail,
    getSubmissionCases: mocks.getSubmissionCases,
  },
}))
vi.mock('@/services/config.service', () => ({
  configService: { getPollSchedule: mocks.getPollSchedule },
}))
vi.mock('@/stores/contestStore', () => ({
  useContestStore: () => ({
    contest: null,
    problems: [],
    whenLoaded: mocks.whenLoaded,
  }),
}))
vi.mock('@/components/editor/CodeEditor.vue', () => ({ default: { template: '<div />' } }))
vi.mock('@/components/common/LoadingSpinner.vue', () => ({
  default: { template: '<div class="loading-stub" />' },
}))
vi.mock('@/components/common/ErrorMessage.vue', () => ({
  default: { props: ['message'], template: '<div>{{ message }}</div>' },
}))

import SubmissionDetailView from '@/views/SubmissionDetailView.vue'

function detailOf(id: string, status: JudgementStatus = 'Accepted'): SubmissionDetail {
  return {
    submitId: id,
    pid: `pid-${id}`,
    displayPid: `HOJ-${id}`,
    username: `user-${id}`,
    submitTime: 1_700_000_000,
    status,
    timeMs: 15,
    memoryKb: 1024,
    score: null,
    length: 20,
    language: 'C++',
    code: 'int main() {}',
    errorMessage: null,
    judger: null,
    oiRankScore: null,
  }
}

function casesOf(count: number): SubmissionCases {
  return {
    cases: Array.from({ length: count }, (_, index) => ({
      caseId: index + 1,
      seq: index + 1,
      status: 'Accepted',
      timeMs: 10,
      memoryKb: 1024,
      score: null,
      groupNum: null,
    })),
    subTasks: [],
    mode: 'default',
  }
}

const mounted: Mounted[] = []
async function mount(): Promise<Mounted> {
  const view = await mountView(SubmissionDetailView)
  mounted.push(view)
  return view
}

async function settle(): Promise<void> {
  await nextTick()
  await settleView(12)
}

beforeEach(() => {
  vi.resetAllMocks()
  route.params = { submitId: 'A' }
  mocks.whenLoaded.mockResolvedValue(undefined)
  mocks.getPollSchedule.mockResolvedValue({ intervalMs: 1_000, timeoutMs: 5_000 })
  mocks.getSubmissionDetail.mockImplementation(async (id: string) => detailOf(id))
  mocks.getSubmissionCases.mockImplementation(async () => casesOf(1))
})

afterEach(() => {
  while (mounted.length > 0) mounted.pop()?.unmount()
  vi.useRealTimers()
  vi.restoreAllMocks()
})

describe('SubmissionDetailView 路由参数切换', () => {
  it('A 详情在途时切到 B：加载 B，迟到 A 不得覆盖或触发 A 测试点请求', async () => {
    const detailA = deferred<SubmissionDetail>()
    const detailB = deferred<SubmissionDetail>()
    mocks.getSubmissionDetail
      .mockImplementationOnce(() => detailA.promise)
      .mockImplementationOnce(() => detailB.promise)
    const view = await mount()

    route.params = { submitId: 'B' }
    await settle()
    expect(view.host.textContent).toContain('提交 #B')
    detailB.resolve(detailOf('B'))
    await settle()
    expect(view.host.textContent).toContain('user-B')

    detailA.resolve(detailOf('A'))
    await settle()
    expect(view.host.textContent).toContain('user-B')
    expect(view.host.textContent).not.toContain('user-A')
    expect(mocks.getSubmissionCases).toHaveBeenCalledWith('B')
    expect(mocks.getSubmissionCases).not.toHaveBeenCalledWith('A')
  })

  it('切换时清空旧详情，迟到 A 测试点不得覆盖 B 测试点', async () => {
    const casesA = deferred<SubmissionCases>()
    const detailB = deferred<SubmissionDetail>()
    mocks.getSubmissionDetail
      .mockImplementationOnce(async () => detailOf('A'))
      .mockImplementationOnce(() => detailB.promise)
    mocks.getSubmissionCases
      .mockImplementationOnce(() => casesA.promise)
      .mockImplementationOnce(async () => casesOf(1))
    const view = await mount()
    expect(view.host.textContent).toContain('user-A')

    route.params = { submitId: 'B' }
    await settle()
    expect(view.host.textContent).toContain('提交 #B')
    expect(view.host.textContent).not.toContain('user-A')

    detailB.resolve(detailOf('B'))
    await settle()
    expect(view.host.textContent).toContain('1/1 通过')
    casesA.resolve(casesOf(2))
    await settle()
    expect(view.host.textContent).toContain('user-B')
    expect(view.host.textContent).toContain('1/1 通过')
    expect(view.host.textContent).not.toContain('2/2 通过')
  })

  it('复制反馈在路由切换时立即复位，不带入下一条提交', async () => {
    vi.useFakeTimers()
    const writeText = vi.fn(async () => {})
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    })
    mocks.getSubmissionDetail.mockImplementationOnce(async () => ({
      ...detailOf('A'),
      errorMessage: 'compile error',
    }))
    const view = await mount()
    const copyButton = [...view.host.querySelectorAll('button')]
      .find((button) => button.textContent?.trim() === '复制')!

    copyButton.click()
    await settle()
    expect(writeText).toHaveBeenCalledWith('compile error')
    expect(copyButton.textContent).toContain('已复制')

    route.params = { submitId: 'B' }
    await settle()
    expect(view.host.textContent).not.toContain('已复制')
  })

  it('旧 poller 停止且新提交重建 deadline：跨过旧截止时刻后 B 仍轮询', async () => {
    vi.useFakeTimers()
    vi.spyOn(Math, 'random').mockReturnValue(0.5)
    mocks.getSubmissionDetail.mockImplementation(async (id: string) => detailOf(id, 'Running'))
    await mount()
    await vi.advanceTimersByTimeAsync(4_000)
    const callsForA = () => mocks.getSubmissionDetail.mock.calls.filter(([id]) => id === 'A').length
    const callsForB = () => mocks.getSubmissionDetail.mock.calls.filter(([id]) => id === 'B').length
    const aBeforeSwitch = callsForA()

    route.params = { submitId: 'B' }
    await settle()
    expect(callsForB()).toBe(1)
    const aAfterSwitch = callsForA()
    expect(aAfterSwitch).toBe(aBeforeSwitch)

    // A 的旧 deadline 在切换后 1s 到期；B 的新 deadline 在切换后 5s 才到期
    await vi.advanceTimersByTimeAsync(2_000)
    expect(callsForA()).toBe(aAfterSwitch)
    expect(callsForB()).toBeGreaterThanOrEqual(3)
  })
})
