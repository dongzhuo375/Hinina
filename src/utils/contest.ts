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
 * 选择登录页、路由守卫与比赛外壳共同使用的权威赛程。
 * 认证比赛仅在与匿名简报属于同一场比赛时优先；否则以简报反映服务端当前赛程。
 */
export function getAuthoritativeSchedule(
  contest: Contest | null | undefined,
  brief: Contest | null | undefined,
): Contest | null {
  if (!contest) return brief ?? null
  if (!brief) return contest
  return contest.id === brief.id ? contest : brief
}

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
