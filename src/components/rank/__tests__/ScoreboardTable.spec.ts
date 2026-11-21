// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest'
import { useContestStore } from '@/stores/contestStore'
import { useRankStore } from '@/stores/rankStore'
import type { Contest } from '@/types/contest'
import type { ContestRankPage, ContestRankRow } from '@/types/rank'
import ScoreboardTable from '@/components/rank/ScoreboardTable.vue'
import { mountView, settleView } from '@/test-support/view-mount'
import type { Mounted } from '@/test-support/view-mount'

/**
 * 回归锁：HOJ 会把当前登录用户前置复制到 records（文档 §9.2），rankStore 装配时
 * 已剔除该副本。表格必须渲染成「置顶我的行 + 纯名次序正文」——若装配层回退
 * （前置副本再次混入 rows），名次列会从「我的名次」开始，本用例变红。
 */

const MY_UID = 'me'

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

function makePage(records: ContestRankRow[], total: number, current = 1): ContestRankPage {
  return { records, total, size: 50, current, pages: Math.max(1, Math.ceil(total / 50)) }
}

let view: Mounted | undefined

/// 挂载组件（共享 helper 供 pinia）后写入比赛上下文与榜单数据，返回榜单 store；
/// store 是响应式的，置数后再 settle 即可驱动重渲染
async function mount(): Promise<ReturnType<typeof useRankStore>> {
  view = await mountView(ScoreboardTable)
  const contestStore = useContestStore()
  contestStore.contest = { id: 'c1', contestType: 0, rankShowName: 'username' } as Contest
  contestStore.problems = []
  const rank = useRankStore()
  await settleView()
  return rank
}

afterEach(() => {
  view?.unmount()
  view = undefined
})

/// 每个 tbody 行首格（名次列）的文本：置顶行带「我」徽章；压缩空白便于断言
function rankCellTexts(): string[] {
  return [...view!.host.querySelectorAll('tbody tr')].map(
    (tr) => tr.querySelector('td')?.textContent?.replace(/\s+/g, '') ?? '',
  )
}

describe('ScoreboardTable — 置顶我的行 + 纯名次序正文', () => {
  it('HOJ 前置副本形态：首行为置顶我（#名次），正文严格按名次 1 起、我在自然位置', async () => {
    const rank = await mount()
    const me = makeRow({ uid: MY_UID, rank: 3, username: 'me' })
    // 服务端形态：me 前置副本 + 自然位置行同页（rank 3 在第 1 页窗口 [1,50] 内）
    rank.applyPage(
      makePage([
        me,
        makeRow({ uid: 'u1', rank: 1, username: 'alice' }),
        makeRow({ uid: 'u2', rank: 2, username: 'bob' }),
        { ...me },
        makeRow({ uid: 'u4', rank: 4, username: 'carol' }),
      ], 120),
      MY_UID,
    )
    await settleView()

    // 名次列序列：置顶我 #3 → 1 → 2 → 3(自然位置) → 4。
    // 若前置副本未被剔除，此处会渲染成 我#3 → #3 → 1 → 2 → 4（首行名次 ≠ 1）
    expect(rankCellTexts()).toEqual(['我#3', '1', '2', '3', '4'])
    // 置顶行与自然位置行并存：我恰好渲染两次（置顶 + 自然位置），缺一不可
    expect(rankCellTexts().filter((text) => text.includes('#3'))).toHaveLength(1)
    expect(rankCellTexts().filter((text) => text === '3')).toHaveLength(1)
  })

  it('第 2 页形态：前置副本是唯一出现 → 只渲染置顶我，正文从本页首名次开始', async () => {
    const rank = await mount()
    const me = makeRow({ uid: MY_UID, rank: 42, username: 'me' })
    rank.applyPage(
      makePage(
        [me, makeRow({ uid: 'u51', rank: 51, username: 'dave' }), makeRow({ uid: 'u52', rank: 52, username: 'erin' })],
        120,
        2,
      ),
      MY_UID,
    )
    await settleView()

    expect(rankCellTexts()).toEqual(['我#42', '51', '52'])
  })

  it('未登录（无 myRow）：正文即纯名次序，无置顶行', async () => {
    const rank = await mount()
    rank.applyPage(
      makePage([
        makeRow({ uid: 'u1', rank: 1, username: 'alice' }),
        makeRow({ uid: 'u2', rank: 2, username: 'bob' }),
      ], 2),
      null,
    )
    await settleView()

    expect(rankCellTexts()).toEqual(['1', '2'])
  })
})
