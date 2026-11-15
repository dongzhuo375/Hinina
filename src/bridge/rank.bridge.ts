import type { ContestRankPage, RankQuery } from '@/types/rank'
import { ipcInvoke } from '@/bridge'

/**
 * 获取比赛排行榜（分页）。
 *
 * 返回的 records 可能含服务端前置的「当前用户」副本，去重与参与人数修正
 * 由 `utils/rank` 的纯函数处理，Bridge 层只做 IPC。
 */
/// `Required<RankQuery>`：HOJ 语义默认值的合并单点在 rank.service（唯一调用方），
/// 必填形态让「绕过 service 直接以可选字段调 bridge」在编译期不可表达
export async function getContestRank(query: Required<RankQuery>): Promise<ContestRankPage> {
  return ipcInvoke<ContestRankPage>('get_contest_rank', {
    contestId: query.contestId,
    currentPage: query.currentPage,
    limit: query.limit,
    keyword: query.keyword,
    removeStar: query.removeStar,
    containsEnd: query.containsEnd,
  })
}
