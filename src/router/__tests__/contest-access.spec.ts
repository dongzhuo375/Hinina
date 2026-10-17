// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import type { Contest } from '@/types/contest'
import { useAuthStore } from '@/stores/authStore'
import { useContestStore } from '@/stores/contestStore'

vi.mock('vue-router', async (importOriginal) => {
  const actual = await importOriginal<typeof import('vue-router')>()
  return { ...actual, createWebHistory: actual.createMemoryHistory }
})
// The access policy is tested on the real router without loading editor/desktop views.
vi.mock('@/views/ContestLayout.vue', () => ({ default: { template: '<div />' } }))
vi.mock('@/views/LoginView.vue', () => ({ default: { template: '<div />' } }))
vi.mock('@/views/ProblemSolveView.vue', () => ({ default: { template: '<div />' } }))
vi.mock('@/views/ProblemSetView.vue', () => ({ default: { template: '<div />' } }))

const schedule: Contest = {
  id: '1', title: '竞赛', startTime: 1_000, endTime: 2_000,
  description: '', contestType: 0, status: 0, auth: 0,
  rankShowName: 'username', sealRank: false, sealRankTime: null,
  allowEndSubmit: false, oiRankScoreType: null,
}

beforeEach(() => {
  setActivePinia(createPinia())
  const auth = useAuthStore()
  auth.sessionResolved = true
  auth.user = { id: '1', username: 'team01', token: 'test-token' }
  useContestStore().brief = { ...schedule }
})

async function openAt(now: number) {
  vi.spyOn(Date, 'now').mockReturnValue(now * 1_000)
  const { default: router } = await import('@/router')
  await router.push({ name: 'Login' })
  await router.push({ name: 'ProblemSolve', params: { displayId: 'A' } })
  return router
}

describe('contest route access', () => {
  it('blocks direct deep links before the start', async () => {
    expect((await openAt(999)).currentRoute.value.name).toBe('Login')
  })
  it('allows authenticated entry at the start', async () => {
    expect((await openAt(1_000)).currentRoute.value.name).toBe('ProblemSolve')
  })
  it('blocks direct deep links after the end', async () => {
    expect((await openAt(2_000)).currentRoute.value.name).toBe('Login')
  })
  it('prefers the authenticated end status to a stale anonymous brief', async () => {
    useContestStore().contest = { ...schedule, status: 1 }
    expect((await openAt(1_500)).currentRoute.value.name).toBe('Login')
  })
  it('uses the current brief when the cached authenticated contest has a different ID', async () => {
    const contest = useContestStore()
    contest.contest = { ...schedule, id: 'old', status: 1 }
    contest.brief = { ...schedule, id: 'current', status: 0 }
    expect((await openAt(1_500)).currentRoute.value.name).toBe('ProblemSolve')
  })
  it('keeps the session guard intact', async () => {
    useAuthStore().user = null
    expect((await openAt(1_500)).currentRoute.value.name).toBe('Login')
  })
  it('loads a missing brief once and stays outside on failure', async () => {
    const contest = useContestStore()
    contest.brief = null
    const load = vi.spyOn(contest, 'loadBrief').mockResolvedValue(undefined)
    expect((await openAt(1_500)).currentRoute.value.name).toBe('Login')
    expect(load).toHaveBeenCalledOnce()
  })
})
