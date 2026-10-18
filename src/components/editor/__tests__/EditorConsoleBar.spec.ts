// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, h, nextTick, ref } from 'vue'
import type { App } from 'vue'
import { createPinia, setActivePinia } from 'pinia'
import type { Pinia } from 'pinia'
import { useContestStore } from '@/stores/contestStore'
import { useSubmissionStore } from '@/stores/submissionStore'
import type { Contest } from '@/types/contest'
import type { SubmissionPage, SubmissionRecord } from '@/types/submission'
import EditorConsoleBar from '@/components/editor/EditorConsoleBar.vue'

const mocks = vi.hoisted(() => ({ listContestSubmissions: vi.fn(), getSubmissionCases: vi.fn(), push: vi.fn() }))
vi.mock('vue-router', () => ({
  useRoute: () => ({ params: { displayId: 'A' } }),
  useRouter: () => ({ push: mocks.push }),
}))
vi.mock('@/services/submission.service', () => ({ submissionService: mocks }))

let app: App | undefined
let host: HTMLDivElement
let pinia: Pinia
const pendingId = ref<string | null>(null)
const problemId = ref('p1')
const oldRecord = {
  submitId: 'old', submitTime: 1_700_000_000, status: 'Accepted', timeMs: 15,
} as SubmissionRecord
const page = (record: SubmissionRecord): SubmissionPage => ({ records: [record], total: 1, size: 1, current: 1, pages: 1 })

async function settle() {
  for (let i = 0; i < 8; i++) await nextTick()
}

async function mount() {
  app = createApp({ render: () => h(EditorConsoleBar, {
    cursor: null, problemId: problemId.value, pendingSubmissionId: pendingId.value,
  }) })
  app.use(pinia)
  app.mount(host)
  await settle()
}

function submitButton() {
  return [...host.querySelectorAll('button')].find((button) => /提交代码|提交中|评测中/.test(button.textContent ?? ''))!
}

beforeEach(() => {
  pinia = createPinia()
  setActivePinia(pinia)
  vi.resetAllMocks()
  mocks.listContestSubmissions.mockResolvedValue(page(oldRecord))
  useContestStore().contest = { id: '1' } as Contest
  pendingId.value = null
  problemId.value = 'p1'
  host = document.createElement('div')
  document.body.append(host)
})

afterEach(() => {
  app?.unmount()
  app = undefined
  host.remove()
})

describe('editor submission feedback', () => {
  it('shows the request state immediately, then keeps the new submission visible over a stale summary', async () => {
    await mount()
    expect(host.textContent).toContain('#old')
    const store = useSubmissionStore()
    store.isSubmitting = true
    await settle()
    expect(submitButton().disabled).toBe(true)
    expect(submitButton().textContent).toContain('提交中')
    expect(host.querySelector('[role="status"]')?.textContent).toContain('正在提交')

    store.submissions.push({ id: 'new', problemId: 'p1', status: 'Pending', submittedAt: '2026-10-18T06:00:00Z' })
    store.isSubmitting = false
    pendingId.value = 'new'
    await settle()
    expect(submitButton().disabled).toBe(true)
    expect(submitButton().textContent).toContain('评测中')
    expect(host.textContent).toContain('#new')
    expect(host.textContent).not.toContain('#old')
    expect(host.querySelector('[role="status"]')?.textContent).toContain('评测结果将自动更新')

    store.submissions[0].status = 'WrongAnswer'
    pendingId.value = null
    await settle()
    expect(submitButton().disabled).toBe(false)
    expect(host.textContent).toContain('#new Wrong Answer')
    expect(host.textContent).not.toContain('#old')
  })

  it('shows the server verdict once the local entry times out to Unknown instead of masking it forever', async () => {
    await mount()
    expect(host.textContent).toContain('#old')
    const store = useSubmissionStore()
    // 复刻轮询超时后的 store 状态：pollOnce 把本地条目收敛为终态 Unknown，
    // 服务端稍后出结果 —— 摘要重拉带回同一提交的终态记录
    mocks.listContestSubmissions.mockResolvedValue(
      page({ ...oldRecord, submitId: 'new', status: 'Accepted', timeMs: 30 }),
    )
    store.submissions.push({ id: 'new', problemId: 'p1', status: 'Unknown', submittedAt: '2026-10-18T06:00:00Z' })
    await settle()
    // 终态 Unknown 不再触发「本地非终态」遮蔽，服务端终态结果接管左 pill
    expect(host.textContent).toContain('#new AC')
    expect(host.textContent).not.toContain('Unknown')
  })

  it('ignores a late summary response after switching problems', async () => {
    let resolveOld!: (value: SubmissionPage) => void
    mocks.listContestSubmissions.mockImplementationOnce(() => new Promise<SubmissionPage>((done) => { resolveOld = done }))
    await mount()
    mocks.listContestSubmissions.mockResolvedValue(page({ ...oldRecord, submitId: 'current' }))
    problemId.value = 'p2'
    await settle()
    expect(host.textContent).toContain('#current')
    resolveOld(page(oldRecord))
    await settle()
    expect(host.textContent).toContain('#current')
    expect(host.textContent).not.toContain('#old')
  })
})
