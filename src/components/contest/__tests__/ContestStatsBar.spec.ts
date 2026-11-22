// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest'
import { useContestStore } from '@/stores/contestStore'
import { useRankStore } from '@/stores/rankStore'
import type { Contest, ContestProblem } from '@/types/contest'
import type { ContestRankRow } from '@/types/rank'
import ContestStatsBar from '@/components/contest/ContestStatsBar.vue'
import { mountView, settleView } from '@/test-support/view-mount'
import type { Mounted } from '@/test-support/view-mount'

/**
 * 回归锁：统计卡三段标签与取值——第三段 ACM 为「总用时」（数值 = HOJ totalTime
 * 含罚时，与榜单「总用时」列同源），OI 为「总得分」；无数据显示 `—` 而非 0。
 */

function makeRow(overrides: Partial<ContestRankRow> = {}): ContestRankRow {
  return {
    rank: 1,
    uid: 'uid-1',
    username: 'alice',
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
    ...overrides,
  }
}

let view: Mounted | undefined

async function mount(options: { contestType?: number; problems?: number } = {}) {
  view = await mountView(ContestStatsBar)
  const contestStore = useContestStore()
  contestStore.contest = { id: 'c1', contestType: options.contestType ?? 0, rankShowName: 'username' } as Contest
  contestStore.problems = Array.from({ length: options.problems ?? 0 }, (_, i) => ({ id: i + 1 } as ContestProblem))
  const rank = useRankStore()
  await settleView()
  return rank
}

afterEach(() => {
  view?.unmount()
  view = undefined
})

/// 三段统计的 { 标签, 数值 }（分隔线 div 不带 px-2，天然被过滤）
function segments(): { label: string; value: string }[] {
  return [...view!.host.querySelectorAll('div.px-2')].map((seg) => ({
    label: seg.querySelector('span')?.textContent?.trim() ?? '',
    value:
      seg.querySelectorAll('span')[1]?.textContent?.replace(/\s+/g, ' ').trim() ?? '',
  }))
}

describe('ContestStatsBar — 三段统计', () => {
  it('ACM：解题进度 / 实时排名 / 总用时（含罚时分钟数，m 后缀）', async () => {
    const rank = await mount({ problems: 12 })
    rank.myRow = makeRow({ ac: 2, rank: 7, totalTime: 2220 })
    rank.participants = 50
    await settleView()

    expect(segments()).toEqual([
      { label: '解题进度', value: '2 / 12 AC' },
      { label: '实时排名', value: '#7 / 50' },
      { label: '总用时', value: '37m' },
    ])
  })

  it('OI：第三段为总得分（totalScore）', async () => {
    const rank = await mount({ contestType: 1, problems: 5 })
    rank.myRow = makeRow({ totalScore: 300, totalTime: 90_000 })
    await settleView()

    expect(segments()[2]).toEqual({ label: '总得分', value: '300' })
  })

  it('myRow 未加载：数值占位 —（不显示 0），第三段标签仍为总用时', async () => {
    await mount({ problems: 12 })

    // 题目数已加载时解题进度保留「— / 12 AC」形态（计数已知、我的 AC 未知）
    expect(segments()).toEqual([
      { label: '解题进度', value: '— / 12 AC' },
      { label: '实时排名', value: '—' },
      { label: '总用时', value: '—' },
    ])
  })
})
