// @vitest-environment jsdom
// ProblemSolveView 组合测试（M2 工作包 C）。
//
// 锁定视图自己的编排职责（store 内部逻辑各有 spec，此处整店打桩）：
// 1. 切题先落盘未保存代码（代码保留是工作区核心承诺）；
// 2. 快速切题的迟到响应污染 store 后，提交守卫拒绝错配且可见（第二道防线）；
// 3. 防错题提交：加载失败后旧题与新路由不配对时拒绝提交（PR21-8）；
// 4. 配对正确时提交确实以正确的题目真实 ID 发生（正向断言，评审 #7）；
// 5. 离开解题页时在途改动落盘。
//
// 异步等待一律用 vi.waitFor 等可观察条件（评审 #9）：视图编排链上每多一个
// await，固定次数的 Promise.resolve 轮询就会读到旧状态，测试变红或假阴性。
import { defineComponent, h, reactive } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mountView } from '@/test-support/view-mount'
import type { Mounted } from '@/test-support/view-mount'

interface Deferred<T> {
  promise: Promise<T>
  resolve: (v: T) => void
  reject: (e: unknown) => void
}
function deferred<T>(): Deferred<T> {
  let resolve!: (v: T) => void
  let reject!: (e: unknown) => void
  const promise = new Promise<T>((res, rej) => { resolve = res; reject = rej })
  return { promise, resolve, reject }
}

const problemOf = (id: string, displayId: string) => ({
  id, title: `题-${displayId}`, description: '', inputDescription: '', outputDescription: '',
  samples: [], timeLimit: 1000, memoryLimit: 256, languages: ['C++'],
})

const route = reactive({ params: { displayId: 'A' } as Record<string, string>, query: {} as Record<string, string> })

/// 供断言顺序与驱动行为的一次性桩仓库（字段对齐视图实际消费面）。
const calls: string[] = []
const storeMocks = vi.hoisted(() => ({
  workspace: {
    code: '', language: 'C++', isDirty: false, isLoadingWorkspace: false,
    loadWorkspace: vi.fn(async () => { calls.push('loadWorkspace') }),
    saveWorkspace: vi.fn(async () => { calls.push('saveWorkspace') }),
    updateCode: vi.fn(), changeLanguage: vi.fn(),
  },
  problem: {
    currentProblem: null as ReturnType<typeof problemOf> | null,
    isLoading: false, error: null as string | null,
    openProblem: vi.fn(async () => { calls.push('openProblem') }),
    loadLimits: vi.fn(), loadMyStatus: vi.fn(),
  },
  submission: {
    isSubmitting: false, pendingFor: vi.fn(() => null), error: null as string | null,
    submitCode: vi.fn(async () => { calls.push('submitCode') }),
  },
}))

vi.mock('vue-router', () => ({
  useRoute: () => route,
  useRouter: () => ({ replace: vi.fn(), push: vi.fn() }),
}))
vi.mock('@/stores/contestStore', () => ({
  useContestStore: () => ({
    contest: { id: 'c1' },
    problems: [
      { displayId: 'A', problemId: 'p-A' },
      { displayId: 'B', problemId: 'p-B' },
    ],
    whenLoaded: vi.fn(async () => {}),
  }),
}))
vi.mock('@/stores/problemStore', () => ({ useProblemStore: () => storeMocks.problem }))
vi.mock('@/stores/workspaceStore', () => ({ useWorkspaceStore: () => storeMocks.workspace }))
vi.mock('@/stores/submissionStore', () => ({
  useSubmissionStore: () => ({
    ...storeMocks.submission,
    // 评审 #14：守卫经 action 上报阻断；实现直写 mock 的 error 字段，
    // 使既有「error 含…」断言继续锁定展示面契约
    reportSubmitBlocked: (message: string) => {
      storeMocks.submission.error = message
    },
  }),
}))
vi.mock('@/services/config.service', () => ({
  configService: {
    getSplitRatio: vi.fn(async () => 0.48),
    updateConfig: vi.fn(async () => {}),
  },
}))

/// CodeEditor 桩：捕获视图绑定的 submit 处理器，测试经它触发提交。
const editorSubmit = { handler: null as (() => void) | null }
vi.mock('@/components/editor/CodeEditor.vue', () => ({
  default: defineComponent({
    setup(_, { attrs }) {
      editorSubmit.handler = attrs.onSubmit as () => void
      return () => h('div', { class: 'editor-stub' })
    },
  }),
}))
vi.mock('@/components/problem/ProblemTabStrip.vue', () => ({ default: { template: '<nav />' } }))
vi.mock('@/components/problem/ProblemStatement.vue', () => ({ default: { template: '<article />' } }))
vi.mock('@/components/editor/EditorConsoleBar.vue', () => ({ default: { template: '<footer />' } }))
vi.mock('@/components/common/LoadingSpinner.vue', () => ({ default: { template: '<div />' } }))
vi.mock('@/components/common/ErrorMessage.vue', () => ({ default: { template: '<div />' } }))

import ProblemSolveView from '@/views/ProblemSolveView.vue'

/// 登记挂载实例，afterEach 统一卸载（评审 #10）：失败用例不残留挂载实例，
/// 避免后续用例经全局 document 查询误触旧实例产生连带失败。
const mounted: Mounted[] = []
async function mount(): Promise<Mounted> {
  const m = await mountView(ProblemSolveView)
  mounted.push(m)
  return m
}

beforeEach(() => {
  vi.clearAllMocks()
  calls.length = 0
  route.params = { displayId: 'A' }
  route.query = {}
  storeMocks.problem.currentProblem = null
  storeMocks.problem.error = null
  storeMocks.workspace.isDirty = false
  storeMocks.submission.error = null
  editorSubmit.handler = null
})

afterEach(() => {
  while (mounted.length > 0) mounted.pop()?.unmount()
})

describe('ProblemSolveView 加载与切题编排', () => {
  it('切题前先落盘未保存代码：saveWorkspace 先于 loadWorkspace', async () => {
    // 初始 A 题加载完成
    storeMocks.problem.openProblem.mockResolvedValueOnce(undefined)
    await mount()
    storeMocks.problem.currentProblem = problemOf('p-A', 'A') // 评审 #8：状态准备不藏在断言里

    calls.length = 0
    storeMocks.workspace.isDirty = true
    storeMocks.problem.openProblem.mockResolvedValueOnce(undefined)
    await mount()
    route.params = { displayId: 'B' } // 触发 watch 切题
    await vi.waitFor(() => {
      expect(calls).toContain('loadWorkspace')
      expect(calls).toContain('saveWorkspace')
    })

    const saveIdx = calls.indexOf('saveWorkspace')
    const loadIdx = calls.indexOf('loadWorkspace')
    expect(loadIdx).toBeGreaterThan(saveIdx) // 保存必须先于新题工作区加载，否则落盘写的是旧内容
  })

  it('配对正确时提交确实发生，且以正确的题目真实 ID（正向断言，评审 #7）', async () => {
    storeMocks.problem.openProblem.mockImplementationOnce(async () => {
      storeMocks.problem.currentProblem = problemOf('p-A', 'A')
    })
    await mount()
    await vi.waitFor(() => {
      expect(storeMocks.problem.currentProblem?.id).toBe('p-A')
    })

    storeMocks.workspace.code = 'code-A'
    editorSubmit.handler?.()
    await vi.waitFor(() => {
      expect(storeMocks.submission.submitCode).toHaveBeenCalledTimes(1)
    })
    const submitArgs = storeMocks.submission.submitCode.mock.calls as unknown as [
      string, string, string, string, string,
    ][]
    // 提交必须携带配对正确的题目真实 ID
    expect(submitArgs[0]?.[1]).toBe('p-A')
    expect(storeMocks.submission.error).toBeNull()
  })

  it('快速切题：迟到旧题响应污染 store 后，提交守卫拒绝错配并给出可见反馈（评审 P1）', async () => {
    // store 现已内置请求序号过期保护（主防线，见 problemStore.spec.ts）；
    // 本测试故意让迟到 mock 复刻「未受保护的写入」，锁定视图层第二道防线的
    // 契约：失败路径错配与防御纵深下，拒绝必须可见而不是静默无响应。
    const lateA = deferred<void>()
    storeMocks.problem.openProblem.mockImplementationOnce(async () => {
      await lateA.promise
      storeMocks.problem.currentProblem = problemOf('p-A', 'A')
    })
    storeMocks.problem.openProblem.mockImplementationOnce(async () => {
      storeMocks.problem.currentProblem = problemOf('p-B', 'B')
    })

    await mount()
    route.params = { displayId: 'B' }
    await vi.waitFor(() => {
      expect(storeMocks.problem.currentProblem?.id).toBe('p-B')
    })
    // B 已就位（配对标记 = B），A 的响应此时才回来并无条件覆盖 store
    lateA.resolve(undefined)
    await vi.waitFor(() => {
      expect(storeMocks.problem.currentProblem?.id).toBe('p-A')
    })

    // 脏状态下提交：displayId 配对仍是 B（第一道校验放行），
    // 但 currentProblem.id 与本次成功加载写入的真实 ID 不一致 —— 必须拒绝，
    // 否则 B 的代码会以 A 的 problem.id 提交；且拒绝必须经控制台条可见
    storeMocks.workspace.code = 'code-B'
    editorSubmit.handler?.()
    await vi.waitFor(() => {
      expect(storeMocks.submission.error).toContain('不一致')
    })
    expect(storeMocks.submission.submitCode).not.toHaveBeenCalled()
  })

  it('防错题提交：openProblem 失败后旧题与新路由不配对，提交被拒绝且可见', async () => {
    // 初始 A 加载成功
    storeMocks.problem.openProblem.mockImplementationOnce(async () => {
      storeMocks.problem.currentProblem = problemOf('p-A', 'A')
    })
    await mount()
    await vi.waitFor(() => {
      expect(storeMocks.problem.currentProblem?.id).toBe('p-A')
    })

    // 切 B：工作区加载成功（code 已是新题），但题目详情加载失败 → currentProblem 仍是 A
    storeMocks.problem.openProblem.mockRejectedValueOnce(new Error('题面 500'))
    route.params = { displayId: 'B' }
    // 等编排可观察信号：切题的 loadWorkspace 已发生（loadWorkspace 出现两次 =
    // 初始加载 + 切题；openProblem 的 reject 路径被 mockRejectedValueOnce 接管，
    // 不经过默认实现，无法以 calls 观察失败本身）
    await vi.waitFor(() => {
      expect(calls.filter((c) => c === 'loadWorkspace')).toHaveLength(2)
    })

    expect(storeMocks.problem.currentProblem?.id).toBe('p-A') // openProblem 失败保留旧题（store 语义）
    storeMocks.workspace.code = 'code-B'
    editorSubmit.handler?.()
    // 旧题与新路由不配对：放行会把新题代码提交到旧题题号（PR21-8）。
    // 此场景 openProblem 未成功，displayId 配对即不成立 —— 命中第一道守卫
    await vi.waitFor(() => {
      expect(storeMocks.submission.error).toContain('切换中')
    })
    expect(storeMocks.submission.submitCode).not.toHaveBeenCalled()
  })

  it('离开解题页（卸载）时在途改动落盘', async () => {
    storeMocks.problem.openProblem.mockResolvedValueOnce(undefined)
    const m = await mount()
    storeMocks.workspace.isDirty = true

    await m.unmount()

    expect(storeMocks.workspace.saveWorkspace).toHaveBeenCalled() // 卸载即离开解题页：必须落盘
  })
})
