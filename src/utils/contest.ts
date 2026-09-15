import type { Contest } from '@/types/contest'

/**
 * 比赛阶段：
 * - `none`     比赛信息缺失（未加载/加载失败/未配置）
 * - `upcoming` 未开始
 * - `running`  进行中
 * - `ended`    已结束
 */
export type ContestPhase = 'none' | 'upcoming' | 'running' | 'ended'

/**
 * 由比赛实体与当前时间推导比赛阶段（纯函数，登录页与顶部栏共用，避免判据分散）。
 *
 * 服务端 `status`（-1=未开始，0=进行中，1=已结束）是列表接口的快照，可能过期，
 * 因此仅在"已结束"上作为权威判据；开始/进行中的切换由本地时钟驱动，
 * 使倒计时归零的瞬间即可触发阶段跃迁（登录页据此自动进入赛场）。
 */
export function getContestPhase(
  contest: Contest | null | undefined,
  nowSecs: number,
): ContestPhase {
  if (!contest) return 'none'
  if (contest.status === 1 || nowSecs >= contest.endTime) return 'ended'
  if (nowSecs >= contest.startTime) return 'running'
  return 'upcoming'
}

/** 比赛是否已开始（进行中或已结束）—— 决定已登录用户能否进入赛场 */
export function hasContestStarted(phase: ContestPhase): boolean {
  return phase === 'running' || phase === 'ended'
}
