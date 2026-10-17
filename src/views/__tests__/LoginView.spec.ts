// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, nextTick } from 'vue'
import type { App } from 'vue'
import { createPinia, setActivePinia } from 'pinia'
import { useAuthStore } from '@/stores/authStore'
import { useContestStore } from '@/stores/contestStore'
import type { Contest } from '@/types/contest'
import LoginView from '@/views/LoginView.vue'

const mocks = vi.hoisted(() => ({
  replace: vi.fn(),
  authService: { login: vi.fn(), logout: vi.fn(), checkSession: vi.fn(), validateSession: vi.fn() },
  contestService: { loadContestBrief: vi.fn() },
}))
vi.mock('vue-router', () => ({ useRouter: () => ({ replace: mocks.replace }) }))
vi.mock('@/services/auth.service', () => ({ authService: mocks.authService }))
vi.mock('@/services/contest.service', () => ({ contestService: mocks.contestService }))

const user = { id: '1', username: 'team01', token: 'test-token' }
const schedule: Contest = {
  id: '1', title: '程序设计竞赛', startTime: 1_010, endTime: 2_000,
  description: '# 比赛须知\n![图](/rules.png)', contestType: 0, status: -1, auth: 0,
  rankShowName: 'username', sealRank: false, sealRankTime: null,
  allowEndSubmit: false, oiRankScoreType: null,
}

let app: App | undefined
let host: HTMLDivElement

async function settle() {
  for (let i = 0; i < 5; i++) await nextTick()
}

function render() {
  const pinia = createPinia()
  setActivePinia(pinia)
  return {
    auth: useAuthStore(),
    contest: useContestStore(),
    async mount() {
      app = createApp(LoginView)
      app.use(pinia)
      app.mount(host)
      await settle()
    },
  }
}

function input(id: string, value: string) {
  const element = host.querySelector<HTMLInputElement>(`#${id}`)!
  element.value = value
  element.dispatchEvent(new Event('input', { bubbles: true }))
}

beforeEach(() => {
  vi.useFakeTimers()
  vi.setSystemTime(1_000_000)
  vi.resetAllMocks()
  vi.spyOn(Math, 'random').mockReturnValue(0.5)
  mocks.contestService.loadContestBrief.mockResolvedValue({ status: 'ok', contest: { ...schedule }, baseUrl: 'https://oj.example' })
  mocks.authService.checkSession.mockResolvedValue(null)
  mocks.authService.login.mockResolvedValue(user)
  mocks.authService.logout.mockResolvedValue(undefined)
  mocks.authService.validateSession.mockResolvedValue('valid')
  host = document.createElement('div')
  document.body.append(host)
})

afterEach(() => {
  app?.unmount()
  app = undefined
  host.remove()
  vi.useRealTimers()
})

describe('login entry lifecycle', () => {
  it('restores a session, waits outside, then enters automatically at the start', async () => {
    mocks.authService.checkSession.mockResolvedValue(user)
    await render().mount()
    expect(host.textContent).toContain('账号已验证')
    expect(host.textContent).not.toMatch(/直接进入赛场|重新加载比赛信息|刷新比赛信息/)
    expect(mocks.replace).not.toHaveBeenCalled()
    await vi.advanceTimersByTimeAsync(10_000)
    expect(mocks.replace).toHaveBeenCalledExactlyOnceWith({ name: 'Contest' })
  })

  it('keeps login errors accessible, supports password visibility and clears the password after success', async () => {
    const state = render()
    await state.mount()
    input('contest-username', ' team01 ')
    input('contest-password', 'secret')
    await settle()
    const reveal = host.querySelector<HTMLButtonElement>('[aria-label="显示密码"]')!
    reveal.click()
    await settle()
    expect(host.querySelector<HTMLInputElement>('#contest-password')!.type).toBe('text')

    mocks.authService.login.mockRejectedValueOnce(new Error('账号或密码错误'))
    host.querySelector('form')!.dispatchEvent(new Event('submit', { cancelable: true }))
    await settle()
    expect(host.querySelector('[role="alert"]')!.textContent).toContain('账号或密码错误')
    expect(host.querySelector('#contest-password')!.getAttribute('aria-invalid')).toBe('true')

    let resolveLogin!: (value: typeof user) => void
    mocks.authService.login.mockReturnValueOnce(new Promise((resolve) => { resolveLogin = resolve }))
    host.querySelector('form')!.dispatchEvent(new Event('submit', { cancelable: true }))
    await settle()
    expect(host.querySelector('button[type="submit"]')!.getAttribute('aria-busy')).toBe('true')
    expect(host.querySelector<HTMLInputElement>('#contest-password')!.disabled).toBe(true)
    resolveLogin(user)
    await settle()
    expect(mocks.authService.login).toHaveBeenLastCalledWith('team01', 'secret')
    expect(host.textContent).toContain('账号已验证')
    expect(mocks.replace).not.toHaveBeenCalled()
    const switchButton = [...host.querySelectorAll('button')].find((b) => b.textContent?.includes('切换参赛账号'))!
    switchButton.click()
    await settle()
    expect(host.querySelector<HTMLInputElement>('#contest-password')!.value).toBe('')
    expect(state.contest.brief?.title).toBe(schedule.title)
  })

  it('renders the original sanitized Markdown brief including relative images', async () => {
    await render().mount()
    host.querySelector<HTMLButtonElement>('[aria-controls="contest-brief"]')!.click()
    await settle()
    expect(host.querySelector('#contest-brief h1')!.textContent).toBe('比赛须知')
    expect(host.querySelector('#contest-brief img')!.getAttribute('src')).toBe('https://oj.example/rules.png')
  })

  it('retries failed brief requests at 2/4/8 seconds, then stops', async () => {
    mocks.contestService.loadContestBrief.mockRejectedValue(new Error('网络异常'))
    await render().mount()
    expect(host.textContent).toContain('正在自动重试')
    for (const delay of [2_000, 4_000, 8_000]) {
      await vi.advanceTimersByTimeAsync(delay)
    }
    expect(mocks.contestService.loadContestBrief).toHaveBeenCalledTimes(4)
    expect(host.textContent).not.toContain('正在自动重试')
    expect(host.textContent).toContain('网络异常')
    await vi.advanceTimersByTimeAsync(60_000)
    expect(mocks.contestService.loadContestBrief).toHaveBeenCalledTimes(4)
  })

  it('cancels brief retry on unmount', async () => {
    mocks.contestService.loadContestBrief.mockRejectedValue(new Error('网络异常'))
    await render().mount()
    app!.unmount()
    app = undefined
    await vi.advanceTimersByTimeAsync(20_000)
    expect(mocks.contestService.loadContestBrief).toHaveBeenCalledTimes(1)
  })

  it('does not reenter an ended contest even when the anonymous brief is stale', async () => {
    const state = render()
    state.auth.user = user
    state.auth.sessionResolved = true
    state.contest.contest = { ...schedule, status: 1 }
    await state.mount()
    expect(host.textContent).toContain('比赛已结束')
    expect(mocks.replace).not.toHaveBeenCalled()
  })

  it('preserves pre-start validation and does not schedule its retry after leaving the page', async () => {
    mocks.contestService.loadContestBrief.mockResolvedValue({ status: 'ok', contest: { ...schedule, startTime: 1_100 }, baseUrl: '' })
    let resolveCheck!: (value: string) => void
    mocks.authService.validateSession.mockReturnValue(new Promise((resolve) => { resolveCheck = resolve }))
    const state = render()
    state.auth.user = user
    state.auth.sessionResolved = true
    await state.mount()
    await vi.advanceTimersByTimeAsync(3_000)
    expect(mocks.authService.validateSession).toHaveBeenCalledTimes(1)
    app!.unmount()
    app = undefined
    resolveCheck('unknown')
    await settle()
    await vi.advanceTimersByTimeAsync(10_000)
    expect(mocks.authService.validateSession).toHaveBeenCalledTimes(1)
  })
})
