import type { ContestRankPage, RankQuery } from '@/types/rank'
import * as rankBridge from '@/bridge/rank.bridge'

/// 榜单默认分页大小（HOJ 建议值：榜单为全量计算后分页，limit 越大单次越慢）
export const DEFAULT_RANK_PAGE_SIZE = 50

/**
 * 榜单服务 — 比赛排行榜的获取与查询参数编排。
 *
 * 不做前端缓存：HOJ 内榜每次实时计算，缓存会给出过期名次；
 * 刷新节奏由 `rankStore` 的轮询器控制（≥10s + 抖动错峰 + 后台暂停）。
 */
export class RankService {
  /**
   * 获取一页榜单。
   *
   * 未显式给出的查询参数按 HOJ 语义补默认值：第 1 页、每页 50 条、
   * 不过滤打星、不含赛后提交（`containsEnd` 仅在比赛 allowEndSubmit 时才真正生效）。
   */
  async getRank(query: Partial<RankQuery> & { contestId: string }): Promise<ContestRankPage> {
    return rankBridge.getContestRank({
      contestId: query.contestId,
      currentPage: query.currentPage ?? 1,
      limit: query.limit ?? DEFAULT_RANK_PAGE_SIZE,
      keyword: query.keyword ?? null,
      removeStar: query.removeStar ?? false,
      containsEnd: query.containsEnd ?? false,
    })
  }
}

export const rankService = new RankService()
