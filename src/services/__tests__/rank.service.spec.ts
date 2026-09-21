import { beforeEach, describe, expect, it, vi } from 'vitest'

/// Bridge 层打桩：service 只依赖 bridge，不需要真实 Tauri 运行时
const { bridge } = vi.hoisted(() => ({
  bridge: {
    getContestRank: vi.fn(),
  },
}))
vi.mock('@/bridge/rank.bridge', () => bridge)

import { DEFAULT_RANK_PAGE_SIZE, rankService } from '@/services/rank.service'

const emptyPage = { records: [], total: 0 }

beforeEach(() => {
  bridge.getContestRank.mockResolvedValue(emptyPage)
})

describe('DEFAULT_RANK_PAGE_SIZE — 榜单默认分页大小的唯一取值点（P71）', () => {
  it('锁定 50', () => {
    // 后端 DEFAULT_RANK_LIMIT 与 RankQuery::default() 是「绕过前端直接调命令」的
    // 防御值，必须与这里同值。漂移没有任何运行时症状（前端恒显式传参），
    // 只能靠两侧用例锁住。对应后端用例：
    // src-tauri/src/commands/tests/mod_tests.rs::rank_default_page_size_matches_frontend_contract
    expect(DEFAULT_RANK_PAGE_SIZE).toBe(50)
  })
})

describe('getRank — 未给出的参数按 HOJ 语义补默认值', () => {
  it('只给 contestId 时补齐第 1 页 / 默认页大小 / 不过滤打星 / 不含赛后提交', async () => {
    await rankService.getRank({ contestId: '1011' })

    expect(bridge.getContestRank).toHaveBeenCalledWith({
      contestId: '1011',
      currentPage: 1,
      limit: DEFAULT_RANK_PAGE_SIZE,
      keyword: null,
      removeStar: false,
      containsEnd: false,
    })
  })

  it('显式给出的参数不得被默认值覆盖（含 0 / false 这类假值）', async () => {
    await rankService.getRank({
      contestId: '1011',
      currentPage: 3,
      limit: 10,
      keyword: 'abc',
      removeStar: true,
      containsEnd: true,
    })

    expect(bridge.getContestRank).toHaveBeenCalledWith({
      contestId: '1011',
      currentPage: 3,
      limit: 10,
      keyword: 'abc',
      removeStar: true,
      containsEnd: true,
    })
  })

  it('原样返回 bridge 结果（service 不做缓存：内榜每次实时计算）', async () => {
    const page = { records: [{ uid: 'u1' }], total: 1 }
    bridge.getContestRank.mockResolvedValue(page)

    await expect(rankService.getRank({ contestId: '1011' })).resolves.toBe(page)
  })
})
