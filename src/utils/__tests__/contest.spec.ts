import { describe, expect, it } from 'vitest'
import { getAuthoritativeSchedule, getContestPhase } from '@/utils/contest'
import type { Contest } from '@/types/contest'

/// 构造比赛实体：仅时间/状态字段有意义，其余取占位值
function makeContest(overrides: Partial<Contest> = {}): Contest {
  return {
    id: '1',
    title: 'Test Contest',
    startTime: 1_000,
    endTime: 2_000,
    description: '',
    contestType: 0,
    status: -1,
    auth: 0,
    rankShowName: 'username',
    sealRank: false,
    sealRankTime: null,
    allowEndSubmit: false,
    oiRankScoreType: null,
    ...overrides,
  }
}

describe('getContestPhase', () => {
  it('比赛信息缺失时为 none', () => {
    expect(getContestPhase(null, 0)).toBe('none')
    expect(getContestPhase(undefined, 0)).toBe('none')
  })

  it('服务端 status=1 优先判定为已结束（即使时间区间未到）', () => {
    expect(getContestPhase(makeContest({ status: 1 }), 0)).toBe('ended')
  })

  it('now < startTime 为未开始', () => {
    expect(getContestPhase(makeContest(), 999)).toBe('upcoming')
  })

  it('startTime <= now < endTime 为进行中（含左边界）', () => {
    expect(getContestPhase(makeContest(), 1_000)).toBe('running')
    expect(getContestPhase(makeContest(), 1_999)).toBe('running')
  })

  it('now >= endTime 为已结束（含右边界）', () => {
    expect(getContestPhase(makeContest(), 2_000)).toBe('ended')
    expect(getContestPhase(makeContest(), 9_999)).toBe('ended')
  })

  it('本地时钟越过 startTime 时以时钟为准（服务端 status 是可能过期的快照）', () => {
    expect(getContestPhase(makeContest({ status: -1 }), 1_500)).toBe('running')
  })
})

describe('getAuthoritativeSchedule', () => {
  it('同一比赛优先使用认证数据', () => {
    const contest = makeContest({ title: '认证数据' })
    const brief = makeContest({ title: '匿名简报' })
    expect(getAuthoritativeSchedule(contest, brief)).toBe(contest)
  })

  it('比赛 ID 不同时采用简报中的当前赛程', () => {
    const staleContest = makeContest({ id: 'old', status: 1 })
    const currentBrief = makeContest({ id: 'current', status: 0 })
    expect(getAuthoritativeSchedule(staleContest, currentBrief)).toBe(currentBrief)
  })

  it('只有一份赛程数据时采用现有数据', () => {
    const contest = makeContest()
    const brief = makeContest({ id: 'current' })
    expect(getAuthoritativeSchedule(null, brief)).toBe(brief)
    expect(getAuthoritativeSchedule(contest, null)).toBe(contest)
    expect(getAuthoritativeSchedule(null, null)).toBeNull()
  })
})
