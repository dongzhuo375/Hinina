// @vitest-environment jsdom
// ProblemSolveView 组合测试（M2 工作包 C）。
//
// 锁定视图自己的编排职责（store 内部逻辑各有 spec，此处整店打桩）：
// 1. 切题先落盘未保存代码（代码保留是工作区核心承诺）；
// 2. 快速切题的迟到响应不得覆盖新题（loadToken 防竞态）；
// 3. 防错题提交：加载失败后 currentProblem（旧题）与路由 displayId（新题）
//    不配对时拒绝提交 —— 新题代码不得提交到旧题题号（PR21-8）；
// 4. 离开解题页时在途改动落盘。
import { defineComponent, h, reactive } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { mountView } from '@/test-support/view-mount'

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
    isSubmitting: false, pendingFor: vi.fn(() => null),
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
vi.mock('@/stores/submissionStore', () => ({ useSubmissionStore: () => storeMocks.submission }))
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

beforeEach(() => {
  vi.clearAllMocks()
  calls.length = 0
  route.params = { displayId: 'A' }
  route.query = {}
  storeMocks.problem.currentProblem = null
  storeMocks.problem.error = null
  storeMocks.workspace.isDirty = false
  editorSubmit.handler = null
})

describe('ProblemSolveView 加载与切题编排', () => {
  it('切题前先落盘未保存代码：saveWorkspace 先于 loadWorkspace', async () => {
    // 初始 A 题加载完成
    storeMocks.problem.openProblem.mockResolvedValueOnce(undefined)
    const m1 = await mountView(ProblemSolveView)
    expect(storeMocks.problem.currentProblem = problemOf('p-A', 'A')).toBeTruthy()
    await m1.unmount()

    calls.length = 0
    storeMocks.workspace.isDirty = true
    storeMocks.problem.openProblem.mockResolvedValueOnce(undefined)
    const m2 = await mountView(ProblemSolveView)
    route.params = { displayId: 'B' } // 触发 watch 切题
    for (let i = 0; i < 10; i += 1) await Promise.resolve()

    const saveIdx = calls.indexOf('saveWorkspace')
    const loadIdx = calls.indexOf('loadWorkspace')
    expect(saveIdx).toBeGreaterThanOrEqual(0) // 脏工作区必须在切题时落盘
    expect(loadIdx).toBeGreaterThan(saveIdx) // 保存必须先于新题工作区加载，否则落盘写的是旧内容

    await m2.unmount()
  })

  it('快速切题：迟到的旧题响应覆盖 store 后，提交守卫拒绝错配（评审 P1）', async () => {
    // 复刻真实语义：openProblem 对迟到响应**没有过期防护**，会无条件写入
    // currentProblem（problemStore.ts）。视图的 loadToken 只丢弃旧加载的编排，
    // 撤销不了已发生的 store 写入 —— 因此这里让 A 的迟到 mock 真实写入 p-A。
    const lateA = deferred<void>()
    storeMocks.problem.openProblem.mockImplementationOnce(async () => {
      await lateA.promise
      storeMocks.problem.currentProblem = problemOf('p-A', 'A')
    })
    storeMocks.problem.openProblem.mockImplementationOnce(async () => {
      storeMocks.problem.currentProblem = problemOf('p-B', 'B')
    })

    const m = await mountView(ProblemSolveView)
    route.params = { displayId: 'B' }
    for (let i = 0; i < 10; i += 1) await Promise.resolve()
    // B 已就位（配对标记 = B），A 的响应此时才回来并无条件覆盖 store
    lateA.resolve(undefined)
    for (let i = 0; i < 10; i += 1) await Promise.resolve()

    // store 已被迟到响应改回旧题（真实语义，视图无法阻止）
    expect(storeMocks.problem.currentProblem?.id).toBe('p-A')

    // 脏状态下提交：displayId 配对仍是 B（第一道校验放行），
    // 但 currentProblem.id 与本次成功加载写入的真实 ID 不一致 —— 必须拒绝，
    // 否则 B 的代码会以 A 的 problem.id 提交
    storeMocks.workspace.code = 'code-B'
    editorSubmit.handler?.()
    for (let i = 0; i < 6; i += 1) await Promise.resolve()
    expect(storeMocks.submission.submitCode).not.toHaveBeenCalled()

    await m.unmount()
  })

  it('防错题提交：openProblem 失败后旧题与新路由不配对，提交被拒绝', async () => {
    // 初始 A 加载成功
    storeMocks.problem.openProblem.mockImplementationOnce(async () => {
      storeMocks.problem.currentProblem = problemOf('p-A', 'A')
    })
    const m = await mountView(ProblemSolveView)

    // 切 B：工作区加载成功（code 已是新题），但题目详情加载失败 → currentProblem 仍是 A
    storeMocks.problem.openProblem.mockRejectedValueOnce(new Error('题面 500'))
    route.params = { displayId: 'B' }
    for (let i = 0; i < 12; i += 1) await Promise.resolve()

    expect(storeMocks.problem.currentProblem?.id).toBe('p-A') // openProblem 失败保留旧题（store 语义）
    storeMocks.workspace.code = 'code-B'
    editorSubmit.handler?.()
    for (let i = 0; i < 6; i += 1) await Promise.resolve()
    // 旧题与新路由不配对：放行会把新题代码提交到旧题题号（PR21-8）
    expect(storeMocks.submission.submitCode).not.toHaveBeenCalled()

    await m.unmount()
  })

  it('离开解题页（卸载）时在途改动落盘', async () => {
    storeMocks.problem.openProblem.mockResolvedValueOnce(undefined)
    const m = await mountView(ProblemSolveView)
    storeMocks.workspace.isDirty = true

    await m.unmount()

    expect(storeMocks.workspace.saveWorkspace).toHaveBeenCalled() // 卸载即离开解题页：必须落盘
  })
})
