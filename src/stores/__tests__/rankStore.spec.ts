import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

/// Service 层打桩：store 只依赖 service，测试不触达 IPC
const { rankService } = vi.hoisted(() => ({
  rankService: { getRank: vi.fn() },
}))
vi.mock('@/services/rank.service', () => ({
  rankService,
  DEFAULT_RANK_PAGE_SIZE: 50,
}))

import { useRankStore } from '@/stores/rankStore'
import type { ContestRankPage, ContestRankRow } from '@/types/rank'

const CONTEST_ID = '1'
const MY_UID = 'me'

/// 构造榜单行，测试只关心被测字段
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

beforeEach(() => {
  setActivePinia(createPinia())
  vi.spyOn(console, 'error').mockImplementation(() => {})
})

describe('applyPage — 服务端响应归一', () => {
  it('按 uid 去重（HOJ 会把当前用户前置复制一份）', () => {
    const rank = useRankStore()
    const me = makeRow({ uid: MY_UID, rank: 42 })
    rank.applyPage(makePage([me, makeRow({ uid: 'u1', rank: 1 }), me], 121), MY_UID)

    expect(rank.rows).toHaveLength(2)
    expect(rank.rows.map((r) => r.uid)).toEqual([MY_UID, 'u1'])
  })

  it('我的行从未去重的原始 records 中定位（前置副本正是它存在的意义）', () => {
    const rank = useRankStore()
    const me = makeRow({ uid: MY_UID, rank: 42, ac: 3, totalTime: 2700 })
    rank.applyPage(makePage([me, makeRow({ uid: 'u1', rank: 1 })], 120), MY_UID)

    expect(rank.myRow?.rank).toBe(42)
    expect(rank.myRow?.ac).toBe(3)
  })

  it('uid 为空时我的行为 null，不抛错', () => {
    const rank = useRankStore()
    rank.applyPage(makePage([makeRow()], 1), null)
    expect(rank.myRow).toBeNull()
  })

  it('参与人数用 total 减去本页重复行数修正', () => {
    const rank = useRankStore()
    const me = makeRow({ uid: MY_UID, rank: 42 })
    rank.applyPage(makePage([me, makeRow({ uid: 'u1', rank: 1 }), me], 361), MY_UID)

    expect(rank.participants).toBe(360)
  })

  it('记录分页信息与刷新时间', () => {
    const rank = useRankStore()
    rank.applyPage(makePage([makeRow()], 120, 3), MY_UID)

    expect(rank.total).toBe(120)
    expect(rank.current).toBe(3)
    expect(rank.pages).toBe(3)
    expect(rank.lastUpdated).toBeGreaterThan(0)
  })
})

describe('loadRank', () => {
  it('成功时填充状态并清除错误', async () => {
    rankService.getRank.mockResolvedValue(makePage([makeRow({ uid: 'u1' })], 1))
    const rank = useRankStore()
    rank.error = '旧错误'

    await rank.loadRank(CONTEST_ID, MY_UID, 1)

    expect(rankService.getRank).toHaveBeenCalledWith({
      contestId: CONTEST_ID,
      currentPage: 1,
      keyword: null,
      removeStar: false,
    })
    expect(rank.rows).toHaveLength(1)
    expect(rank.error).toBeNull()
    expect(rank.isLoading).toBe(false)
    expect(rank.contestId).toBe(CONTEST_ID)
    expect(rank.uid).toBe(MY_UID)
  })

  it('失败时记录原因、复位 loading 并向上抛出', async () => {
    rankService.getRank.mockRejectedValue(new Error('网络异常'))
    const rank = useRankStore()

    await expect(rank.loadRank(CONTEST_ID, MY_UID)).rejects.toThrow('网络异常')
    expect(rank.error).toBe('网络异常')
    expect(rank.isLoading).toBe(false)
  })

  it('失败时保留上一次数据（轮询失败不该清空榜单）', async () => {
    const rank = useRankStore()
    rankService.getRank.mockResolvedValue(makePage([makeRow({ uid: 'u1' })], 1))
    await rank.loadRank(CONTEST_ID, MY_UID)
    expect(rank.rows).toHaveLength(1)

    rankService.getRank.mockRejectedValue(new Error('超时'))
    await expect(rank.loadRank(CONTEST_ID, MY_UID)).rejects.toThrow()
    expect(rank.rows).toHaveLength(1)
  })
})

describe('refresh — 轮询任务', () => {
  it('吞掉异常，避免打断轮询节奏', async () => {
    rankService.getRank.mockRejectedValue(new Error('超时'))
    const rank = useRankStore()
    rank.contestId = CONTEST_ID
    rank.uid = MY_UID

    await expect(rank.refresh()).resolves.toBeUndefined()
    expect(rank.error).toBe('超时')
  })

  it('未设置 contestId 时不发请求', async () => {
    const rank = useRankStore()
    await rank.refresh()
    expect(rankService.getRank).not.toHaveBeenCalled()
  })
})

describe('分页与筛选', () => {
  it('setPage 越界时不发请求', async () => {
    const rank = useRankStore()
    rank.contestId = CONTEST_ID
    rank.pages = 3

    await rank.setPage(0)
    await rank.setPage(4)
    expect(rankService.getRank).not.toHaveBeenCalled()
  })

  it('setPage 在范围内时按页请求', async () => {
    rankService.getRank.mockResolvedValue(makePage([], 0, 2))
    const rank = useRankStore()
    rank.contestId = CONTEST_ID
    rank.pages = 3

    await rank.setPage(2)
    expect(rankService.getRank).toHaveBeenCalledWith(
      expect.objectContaining({ currentPage: 2 }),
    )
  })

  it('setKeyword 去除首尾空白并回到第 1 页', async () => {
    rankService.getRank.mockResolvedValue(makePage([], 0))
    const rank = useRankStore()
    rank.contestId = CONTEST_ID
    rank.current = 3

    await rank.setKeyword('  XX大学  ')
    expect(rank.keyword).toBe('XX大学')
    expect(rankService.getRank).toHaveBeenCalledWith(
      expect.objectContaining({ keyword: 'XX大学', currentPage: 1 }),
    )
  })

  it('空关键词以 null 传给服务端（避免空串被当成搜索条件）', async () => {
    rankService.getRank.mockResolvedValue(makePage([], 0))
    const rank = useRankStore()
    rank.contestId = CONTEST_ID

    await rank.setKeyword('   ')
    expect(rankService.getRank).toHaveBeenCalledWith(
      expect.objectContaining({ keyword: null }),
    )
  })

  it('切到「正式参赛队」置 removeStar 并重新请求（服务端参数）', async () => {
    rankService.getRank.mockResolvedValue(makePage([], 0))
    const rank = useRankStore()
    rank.contestId = CONTEST_ID

    await rank.setGroupFilter('official')
    expect(rank.removeStar).toBe(true)
    expect(rankService.getRank).toHaveBeenCalledWith(
      expect.objectContaining({ removeStar: true, currentPage: 1 }),
    )
  })

  it('切到打星/女生队进入全量快照模式（跨页过滤，见下方专节）', async () => {
    // 旧语义「仅客户端过滤当前页、不发请求」已废弃：那样跨页会漏行。
    // 现在 setGroupFilter('star'|'female') 触发 fetchAllRows 全量拉取，
    // 详细断言见「全量快照模式」describe。
    rankService.getRank.mockResolvedValue(makePage([makeRow({ uid: 'star', rank: -1 })], 1))
    const rank = useRankStore()
    rank.contestId = CONTEST_ID

    await rank.setGroupFilter('star')
    expect(rankService.getRank).toHaveBeenCalled()
    expect(rank.isFullMode).toBe(true)
  })

  it('从 official 切回 all 会复位 removeStar 并重新请求', async () => {
    rankService.getRank.mockResolvedValue(makePage([], 0))
    const rank = useRankStore()
    rank.contestId = CONTEST_ID
    rank.removeStar = true

    await rank.setGroupFilter('all')
    expect(rank.removeStar).toBe(false)
    expect(rankService.getRank).toHaveBeenCalledTimes(1)
  })
})

describe('startLive / stopLive — 实时刷新', () => {
  it('启动后按 10s±2s 抖动周期刷新，停止后不再刷新', async () => {
    vi.useFakeTimers()
    try {
      rankService.getRank.mockResolvedValue(makePage([makeRow()], 1))
      const rank = useRankStore()
      rank.contestId = CONTEST_ID
      rank.uid = MY_UID

      rank.startLive()
      expect(rank.isLive).toBe(true)

      // 轮询器首次执行在一个完整周期之后（首屏数据由视图自行拉取）
      expect(rankService.getRank).not.toHaveBeenCalled()

      // 抖动上限 2s：推进 12s 必然已触发至少一次
      await vi.advanceTimersByTimeAsync(12_000)
      expect(rankService.getRank).toHaveBeenCalled()

      const callsAfterStop = rankService.getRank.mock.calls.length
      rank.stopLive()
      expect(rank.isLive).toBe(false)

      await vi.advanceTimersByTimeAsync(60_000)
      expect(rankService.getRank).toHaveBeenCalledTimes(callsAfterStop)
    } finally {
      vi.useRealTimers()
    }
  })

  it('重复 startLive 不会产生多个轮询器', async () => {
    vi.useFakeTimers()
    try {
      rankService.getRank.mockResolvedValue(makePage([], 0))
      const rank = useRankStore()
      rank.contestId = CONTEST_ID

      rank.startLive()
      rank.startLive()
      rank.startLive()

      await vi.advanceTimersByTimeAsync(12_000)
      // 若存在多个轮询器，同一周期会发出多次请求
      expect(rankService.getRank).toHaveBeenCalledTimes(1)
      rank.stopLive()
    } finally {
      vi.useRealTimers()
    }
  })

  it('调用方给出的暂停判据生效时跳过请求但保持轮询存活', async () => {
    vi.useFakeTimers()
    try {
      rankService.getRank.mockResolvedValue(makePage([], 0))
      const rank = useRankStore()
      rank.contestId = CONTEST_ID

      let paused = true
      rank.startLive(() => paused)

      await vi.advanceTimersByTimeAsync(12_000)
      expect(rankService.getRank).not.toHaveBeenCalled()

      // 解除暂停（如比赛重新进入进行中）后应恢复刷新
      paused = false
      await vi.advanceTimersByTimeAsync(12_000)
      expect(rankService.getRank).toHaveBeenCalled()
      expect(rank.isLive).toBe(true)
      rank.stopLive()
    } finally {
      vi.useRealTimers()
    }
  })

  it('stopLive 幂等', () => {
    const rank = useRankStore()
    expect(() => {
      rank.stopLive()
      rank.stopLive()
    }).not.toThrow()
    expect(rank.isLive).toBe(false)
  })
})

describe('全量快照模式 — 打星/女生队跨页过滤', () => {
  /// 三页服务端响应：每页都前置复制一份「我」（文档 §9.2），跨页合并须去重
  function mockThreePages() {
    const me = makeRow({ uid: MY_UID, rank: 42 })
    rankService.getRank.mockImplementation(async (q: { currentPage?: number }) => {
      const current = q.currentPage ?? 1
      if (current === 1) {
        return makePage(
          [me, makeRow({ uid: 'starA', rank: -1 }), makeRow({ uid: 'u1', rank: 1 })],
          120,
          1,
        )
      }
      if (current === 2) {
        return makePage(
          [
            me,
            makeRow({ uid: 'u2', rank: 2 }),
            makeRow({ uid: 'f1', rank: 3, gender: 'female' }),
          ],
          120,
          2,
        )
      }
      return makePage([me, makeRow({ uid: 'starB', rank: -1 })], 120, 3)
    })
  }

  it('setGroupFilter(star) 顺序拉取全部页并按 uid 跨页去重合并', async () => {
    mockThreePages()
    const rank = useRankStore()
    rank.contestId = CONTEST_ID
    rank.uid = MY_UID

    await rank.setGroupFilter('star')

    expect(rankService.getRank).toHaveBeenCalledTimes(3)
    // 快照必须包含打星行（removeStar=false），否则 star 筛选恒为空
    expect(rankService.getRank).toHaveBeenCalledWith(
      expect.objectContaining({ removeStar: false, limit: 50 }),
    )
    expect(rank.fullFetchState).toBe('done')
    // me/starA/u1/u2/f1/starB —— me 的每页前置副本被去重
    expect(rank.fullLoadedRows).toBe(6)
    expect(rank.isFullMode).toBe(true)
    expect(rank.visibleRows.map((r) => r.uid)).toEqual(['starA', 'starB'])
    expect(rank.pages).toBe(1)
    expect(rank.current).toBe(1)
  })

  it('star ↔ female 互切重拉快照并按新分组过滤', async () => {
    mockThreePages()
    const rank = useRankStore()
    rank.contestId = CONTEST_ID

    await rank.setGroupFilter('star')
    await rank.setGroupFilter('female')

    expect(rankService.getRank).toHaveBeenCalledTimes(6)
    expect(rank.visibleRows.map((r) => r.uid)).toEqual(['f1'])
    expect(rank.groupFilter).toBe('female')
  })

  it('全量模式下翻页是纯客户端切片，不发请求', async () => {
    const stars = Array.from({ length: 120 }, (_, i) => makeRow({ uid: `s${i}`, rank: -1 }))
    rankService.getRank.mockResolvedValue(makePage(stars, 120, 1))
    const rank = useRankStore()
    rank.contestId = CONTEST_ID

    await rank.setGroupFilter('star')
    expect(rank.pages).toBe(3) // ceil(120/50)
    expect(rank.visibleRows).toHaveLength(50)

    const callsBefore = rankService.getRank.mock.calls.length
    await rank.setPage(2)
    expect(rankService.getRank).toHaveBeenCalledTimes(callsBefore)
    expect(rank.current).toBe(2)
    expect(rank.fullPage).toBe(2)
    expect(rank.visibleRows).toHaveLength(50)
    expect(rank.visibleRows[0].uid).toBe('s50')

    await rank.setPage(3)
    expect(rank.visibleRows).toHaveLength(20)

    await rank.setPage(99) // 越界忽略
    expect(rank.current).toBe(3)
  })

  it('服务端页数超过 40 页上限时截断并标记 truncated', async () => {
    rankService.getRank.mockImplementation(async (q: { currentPage?: number }) => {
      const current = q.currentPage ?? 1
      const records = Array.from({ length: 50 }, (_, i) =>
        makeRow({ uid: `u${(current - 1) * 50 + i}`, rank: (current - 1) * 50 + i + 1 }),
      )
      return makePage(records, 5000, current) // pages = 100
    })
    const rank = useRankStore()
    rank.contestId = CONTEST_ID

    await rank.setGroupFilter('star')

    expect(rankService.getRank).toHaveBeenCalledTimes(40)
    expect(rank.fullFetchState).toBe('truncated')
    expect(rank.fullLoadedRows).toBe(2000)
  })

  it('全量模式下自动轮询暂停，refresh() 手动触发重拉快照', async () => {
    vi.useFakeTimers()
    try {
      rankService.getRank.mockResolvedValue(
        makePage([makeRow({ uid: 'star', rank: -1 })], 1, 1),
      )
      const rank = useRankStore()
      rank.contestId = CONTEST_ID
      rank.uid = MY_UID

      rank.startLive()
      await rank.setGroupFilter('star')
      const callsAfterFilter = rankService.getRank.mock.calls.length

      // 全量快照太重，60s 内轮询器不得发出任何请求
      await vi.advanceTimersByTimeAsync(60_000)
      expect(rankService.getRank).toHaveBeenCalledTimes(callsAfterFilter)

      // 手动刷新 → 重拉快照
      await rank.refresh()
      expect(rankService.getRank.mock.calls.length).toBeGreaterThan(callsAfterFilter)

      // 切回全场总榜 → 恢复正常轮询
      await rank.setGroupFilter('all')
      const callsAfterExit = rankService.getRank.mock.calls.length
      await vi.advanceTimersByTimeAsync(12_000)
      expect(rankService.getRank.mock.calls.length).toBeGreaterThan(callsAfterExit)
      rank.stopLive()
    } finally {
      vi.useRealTimers()
    }
  })

  it('切回 all/official 清空快照并恢复服务端分页', async () => {
    rankService.getRank.mockResolvedValue(
      makePage([makeRow({ uid: 'star', rank: -1 })], 1, 1),
    )
    const rank = useRankStore()
    rank.contestId = CONTEST_ID

    await rank.setGroupFilter('star')
    expect(rank.fullRows).not.toBeNull()

    await rank.setGroupFilter('all')
    expect(rank.fullRows).toBeNull()
    expect(rank.fullFetchState).toBe('idle')
    expect(rank.fullLoadedRows).toBe(0)
    expect(rank.isFullMode).toBe(false)
    // 退出全量模式时 pages/current 已被客户端分页覆写，即使 removeStar 未变也须重载第 1 页
    expect(rankService.getRank).toHaveBeenLastCalledWith(
      expect.objectContaining({ currentPage: 1, removeStar: false }),
    )
  })

  it('全量模式下 setKeyword 携带关键词重拉快照并回到第 1 页', async () => {
    rankService.getRank.mockResolvedValue(
      makePage([makeRow({ uid: 'star', rank: -1 })], 1, 1),
    )
    const rank = useRankStore()
    rank.contestId = CONTEST_ID

    await rank.setGroupFilter('star')
    await rank.setKeyword('  XX大学  ')

    expect(rank.keyword).toBe('XX大学')
    expect(rank.fullPage).toBe(1)
    expect(rankService.getRank).toHaveBeenLastCalledWith(
      expect.objectContaining({ keyword: 'XX大学', currentPage: 1, removeStar: false }),
    )
  })

  it('快照拉取失败且无旧快照时标记 error，visibleRows 退化为筛当前页', async () => {
    rankService.getRank.mockRejectedValue(new Error('网络异常'))
    const rank = useRankStore()
    rank.contestId = CONTEST_ID
    rank.rows = [makeRow({ uid: 'star', rank: -1 }), makeRow({ uid: 'u1', rank: 1 })]

    await expect(rank.setGroupFilter('star')).rejects.toThrow('网络异常')
    expect(rank.fullFetchState).toBe('error')
    expect(rank.fullRows).toBeNull()
    expect(rank.isFullMode).toBe(false)
    expect(rank.error).toBe('网络异常')
    // 退化：仍按当前页客户端过滤，聊胜于无
    expect(rank.visibleRows.map((r) => r.uid)).toEqual(['star'])
  })

  it('重拉失败时保留旧快照继续浏览（状态不回退）', async () => {
    rankService.getRank.mockResolvedValue(
      makePage([makeRow({ uid: 'star', rank: -1 })], 1, 1),
    )
    const rank = useRankStore()
    rank.contestId = CONTEST_ID

    await rank.setGroupFilter('star')
    expect(rank.fullFetchState).toBe('done')

    rankService.getRank.mockRejectedValue(new Error('超时'))
    await rank.refresh() // refresh 吞异常
    expect(rank.fullRows).not.toBeNull()
    expect(rank.fullFetchState).toBe('done')
    expect(rank.error).toBe('超时')
    expect(rank.visibleRows.map((r) => r.uid)).toEqual(['star'])
  })

  it('未设置 contestId 时不发请求', async () => {
    const rank = useRankStore()
    await rank.setGroupFilter('star')
    expect(rankService.getRank).not.toHaveBeenCalled()
  })
})
