// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mountView } from '@/test-support/view-mount'
import type { Mounted } from '@/test-support/view-mount'
import type { Contest } from '@/types/contest'

const mocks = vi.hoisted(() => ({
  getRank: vi.fn(),
  loadConfiguredContest: vi.fn(),
}))

vi.mock('@/services/rank.service', () => ({
  rankService: { getRank: mocks.getRank },
  DEFAULT_RANK_PAGE_SIZE: 50,
}))
vi.mock('@/services/contest.service', () => ({
  contestService: { loadConfiguredContest: mocks.loadConfiguredContest },
}))
vi.mock('@/components/common/PageHeading.vue', () => ({
  default: { template: '<section><slot /><slot name="description" /></section>' },
}))
vi.mock('@/components/common/PagePager.vue', () => ({ default: { template: '<div />' } }))
vi.mock('@/components/common/ErrorMessage.vue', () => ({ default: { template: '<div />' } }))
vi.mock('@/components/common/LoadingSpinner.vue', () => ({ default: { template: '<div />' } }))
vi.mock('@/components/contest/ContestStatsBar.vue', () => ({ default: { template: '<div />' } }))
vi.mock('@/components/rank/RankToolbar.vue', () => ({ default: { template: '<div />' } }))
vi.mock('@/components/rank/ScoreboardTable.vue', () => ({ default: { template: '<div />' } }))

import RankView from '@/views/RankView.vue'
import { useAuthStore } from '@/stores/authStore'
import { useContestStore } from '@/stores/contestStore'
import { useRankStore } from '@/stores/rankStore'
import type { ContestRankPage } from '@/types/rank'

function makeContest(id: string): Contest {
  return {
    id,
    title: `比赛 ${id}`,
    startTime: 1_000,
    endTime: 9_999_999_999,
    description: '',
    contestType: 0,
    status: 0,
    auth: 0,
    rankShowName: 'username',
    sealRank: false,
    sealRankTime: null,
    allowEndSubmit: false,
    oiRankScoreType: null,
  }
}

function makeRankPage(contestId: string): ContestRankPage {
  return {
    records: [{
      rank: -1,
      uid: `${contestId}-star`,
      username: `star-${contestId}`,
      realname: '',
      nickname: '',
      school: '',
      gender: '',
      avatar: '',
      ac: 0,
      total: 0,
      totalTime: 0,
      totalScore: null,
      submissionInfo: {},
      timeInfo: {},
    }],
    total: 1,
    size: 50,
    current: 1,
    pages: 1,
  }
}

const mounted: Mounted[] = []

beforeEach(() => {
  vi.resetAllMocks()
  mocks.loadConfiguredContest.mockResolvedValue({
    contest: makeContest('A'),
    problems: [],
  })
  mocks.getRank.mockImplementation(async (query: { contestId: string }) =>
    makeRankPage(query.contestId))
})

afterEach(() => {
  while (mounted.length > 0) mounted.pop()?.unmount()
  useRankStore().stopLive()
  vi.restoreAllMocks()
})

describe('RankView 比赛切换', () => {
  it('A 全量筛选后外壳切到 B：按 B 重拉快照并停掉旧轮询', async () => {
    const view = await mountView(RankView)
    mounted.push(view)
    const contestStore = useContestStore()
    const rankStore = useRankStore()
    useAuthStore().user = { id: 'u1', username: 'team01', token: 'tk' }

    await vi.waitFor(() => expect(contestStore.contest?.id).toBe('A'))
    await vi.waitFor(() => expect(rankStore.contestId).toBe('A'))
    await rankStore.setGroupFilter('star')
    expect(rankStore.fullRows?.map((row) => row.uid)).toEqual(['A-star'])

    const callsBeforeSwitch = mocks.getRank.mock.calls.length
    contestStore.contest = makeContest('B')
    await vi.waitFor(() => expect(rankStore.contestId).toBe('B'))
    await vi.waitFor(() =>
      expect(rankStore.fullRows?.map((row) => row.uid)).toEqual(['B-star']))

    const switchedCalls = mocks.getRank.mock.calls.slice(callsBeforeSwitch)
    expect(switchedCalls.every(([query]) => query.contestId === 'B')).toBe(true)
    expect(switchedCalls.length).toBeGreaterThan(0)
  })
})
