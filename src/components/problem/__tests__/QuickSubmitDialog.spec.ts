// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, nextTick } from 'vue'
import type { App } from 'vue'
import { createPinia, setActivePinia } from 'pinia'
import type { Pinia } from 'pinia'
import { useContestStore } from '@/stores/contestStore'
import { useSubmissionStore } from '@/stores/submissionStore'
import type { Contest, ContestProblem } from '@/types/contest'
import QuickSubmitDialog from '@/components/problem/QuickSubmitDialog.vue'

const mocks = vi.hoisted(() => ({
  submitCode: vi.fn(), pollJudgement: vi.fn(), getProblem: vi.fn(), push: vi.fn(),
}))
vi.mock('vue-router', () => ({ useRouter: () => ({ push: mocks.push }) }))
vi.mock('@/services/submission.service', () => ({ submissionService: mocks }))
vi.mock('@/services/problem.service', () => ({ problemService: mocks }))
vi.mock('@/services/config.service', () => ({ configService: {
  getPollSchedule: async () => ({ intervalMs: 2_000, timeoutMs: 10_000 }),
} }))
vi.mock('@/components/editor/CodeEditor.vue', () => ({ default: {
  props: ['modelValue'], emits: ['update:modelValue', 'submit'],
  template: '<textarea :value="modelValue" @input="$emit(\'update:modelValue\', $event.target.value)" @keydown.ctrl.enter.prevent="$emit(\'submit\')" />',
} }))

const contest = { id: '1' } as Contest
const problem: ContestProblem = { id: 1, cid: '1', displayId: 'A', displayTitle: '两数之和', problemId: 'p1', ac: 0, total: 0, color: '' }
let app: App | undefined
let pinia: Pinia
let host: HTMLDivElement

async function settle() {
  for (let i = 0; i < 8; i++) await nextTick()
}

async function mount() {
  app = createApp(QuickSubmitDialog, { problem })
  app.use(pinia)
  app.mount(host)
  await settle()
  const input = host.querySelector('textarea')!
  input.value = 'int main(){}'
  input.dispatchEvent(new Event('input', { bubbles: true }))
  await settle()
  return input
}

function submitButton() {
  return [...host.querySelectorAll('button')].find((button) => /提交代码|提交中|评测中/.test(button.textContent ?? ''))!
}

function shortcut(input: HTMLTextAreaElement) {
  input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', ctrlKey: true, bubbles: true, cancelable: true }))
}

beforeEach(() => {
  pinia = createPinia()
  setActivePinia(pinia)
  vi.resetAllMocks()
  mocks.getProblem.mockResolvedValue({ languages: ['C++'] })
  mocks.pollJudgement.mockResolvedValue({ status: 'Pending' })
  useContestStore().contest = { ...contest }
  host = document.createElement('div')
  document.body.append(host)
})

afterEach(() => {
  app?.unmount()
  app = undefined
  useSubmissionStore().stopAllPolling()
  host.remove()
})

describe('quick submission feedback', () => {
  it('shows progress immediately and prevents mouse/keyboard repeats through judging', async () => {
    let resolve!: (id: string) => void
    mocks.submitCode.mockImplementationOnce(() => new Promise<string>((done) => { resolve = done }))
    const input = await mount()
    submitButton().click()
    shortcut(input)
    await settle()
    expect(submitButton().disabled).toBe(true)
    expect(submitButton().textContent).toContain('提交中')
    expect(host.querySelector('[role="status"]')?.textContent).toContain('正在提交')
    expect(mocks.submitCode).toHaveBeenCalledTimes(1)

    resolve('s1')
    await settle()
    expect(submitButton().disabled).toBe(true)
    expect(submitButton().textContent).toContain('评测中')
    expect(host.textContent).toContain('已提交 #s1')
    submitButton().click()
    shortcut(input)
    expect(mocks.submitCode).toHaveBeenCalledTimes(1)

    // Editing the payload permits a deliberate new submission while the old one is judged.
    input.value = 'int main(){ return 0; }'
    input.dispatchEvent(new Event('input', { bubbles: true }))
    await settle()
    expect(submitButton().disabled).toBe(false)

    // Returning to the original payload restores its pending submission protection.
    input.value = 'int main(){}'
    input.dispatchEvent(new Event('input', { bubbles: true }))
    await settle()
    expect(submitButton().disabled).toBe(true)
    mocks.pollJudgement.mockResolvedValue({ status: 'WrongAnswer' })
    await useSubmissionStore().pollOnce('s1')
    await settle()
    expect(submitButton().disabled).toBe(false)
    expect(host.textContent).toContain('Wrong Answer')
  })

  it('locks before waiting for contest data, so shortcuts cannot queue multiple requests', async () => {
    const store = useContestStore()
    store.contest = null
    let resolve!: () => void
    const loading = vi.spyOn(store, 'whenLoaded').mockImplementation(() => new Promise<void>((done) => { resolve = done }))
    mocks.submitCode.mockResolvedValue('s1')
    const input = await mount()
    submitButton().click()
    shortcut(input)
    await settle()
    expect(loading).toHaveBeenCalledTimes(1)
    expect(submitButton().disabled).toBe(true)
    expect(mocks.submitCode).not.toHaveBeenCalled()
    store.contest = { ...contest }
    resolve()
    await settle()
    expect(mocks.submitCode).toHaveBeenCalledTimes(1)
  })

  it('shows request errors and unlocks for a deliberate retry', async () => {
    mocks.submitCode.mockRejectedValueOnce(new Error('网络异常'))
    await mount()
    submitButton().click()
    await settle()
    expect(host.querySelector('[role="alert"]')?.textContent).toBe('网络异常')
    expect(submitButton().disabled).toBe(false)
  })
})
