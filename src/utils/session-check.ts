/**
 * 赛前会话预检的调度策略（纯函数，随机源可注入以便测试）。
 *
 * 背景：全场客户端若在同一时刻校验会话，会在开赛前给 OJ 服务器制造同步尖峰。
 * 因此把预检点随机散布在窗口内（错峰），并区分三种情形：
 *
 * ```
 * 距开赛 > 10min          → periodic：每 5min ± 60s 周期复检（长等待场景）
 * 10min ≥ 距开赛 > 3min   → window  ：在剩余窗口内随机取点，一次性预检
 * 距开赛 ≤ 3min           → immediate：迟到启动等场景，0–3s 抖动后立即预检
 * 距开赛 ≤ 30s            → null    ：不再预检，交给进场流程与全局 401 兜底
 * ```
 *
 * 注意：进场（T-0 导航）本身**不做**错峰 —— 准点进场是公平性要求，
 * 且此时服务端峰值来自赛制本身，无法也不应削平。
 */

/// 预检窗口左边界：距开赛 10 分钟进入窗口
export const PRECHECK_WINDOW_START_MS = 10 * 60 * 1000
/// 预检窗口右边界：最晚在开赛前 3 分钟完成预检，留出重新登录的缓冲
export const PRECHECK_WINDOW_END_MS = 3 * 60 * 1000
/// 停止预检阈值：距开赛不足 30 秒时不再校验
export const PRECHECK_STOP_MS = 30 * 1000
/// 长等待时的周期复检间隔
export const PERIODIC_RECHECK_MS = 5 * 60 * 1000
/// 周期复检抖动幅度（±60s），避免同一批开机的客户端同步复检
export const PERIODIC_JITTER_MS = 60 * 1000
/// 立即预检时的抖动上限（0–3s）
export const IMMEDIATE_JITTER_MS = 3 * 1000
/// `unknown`（网络异常）后的单次重试退避区间
export const RETRY_BACKOFF_MIN_MS = 2 * 1000
export const RETRY_BACKOFF_MAX_MS = 5 * 1000

/// 预检类型：决定本次校验后是否继续排程（仅 periodic 继续）
export type PrecheckReason = 'periodic' | 'window' | 'immediate'

/// 一次预检排程结果
export interface PrecheckPlan {
  /** 相对当前的延迟毫秒数 */
  delayMs: number
  reason: PrecheckReason
}

/**
 * 计算下一次会话预检的排程；返回 `null` 表示不应再预检。
 *
 * @param nowMs   当前时间（毫秒）
 * @param startMs 比赛开始时间（毫秒）
 * @param rand    随机源，默认 `Math.random`，测试可注入确定值
 */
export function planNextPrecheck(
  nowMs: number,
  startMs: number,
  rand: () => number = Math.random,
): PrecheckPlan | null {
  const untilStart = startMs - nowMs

  // 已开赛或临近开赛：预检已无意义，进场后由全局 401 兜底
  if (untilStart <= PRECHECK_STOP_MS) return null

  // 尚未进入窗口：周期复检，且不越过窗口左边界（到点后重新规划为窗口内随机取点）
  if (untilStart > PRECHECK_WINDOW_START_MS) {
    const jitter = (rand() * 2 - 1) * PERIODIC_JITTER_MS
    const delay = Math.min(PERIODIC_RECHECK_MS + jitter, untilStart - PRECHECK_WINDOW_START_MS)
    return { delayMs: Math.max(0, delay), reason: 'periodic' }
  }

  // 窗口内：在 [now, T-3min] 剩余区间随机取点，实现全场错峰
  if (untilStart > PRECHECK_WINDOW_END_MS) {
    return { delayMs: rand() * (untilStart - PRECHECK_WINDOW_END_MS), reason: 'window' }
  }

  // 错过窗口（迟到启动等）：立即执行，附小抖动避免同批客户端齐发
  return { delayMs: rand() * IMMEDIATE_JITTER_MS, reason: 'immediate' }
}

/**
 * `unknown` 结果后的重试延迟（2–5s 随机）。
 *
 * 只重试一次：网络异常通常是机房链路问题，重试风暴只会加重拥塞。
 */
export function planRetryDelayMs(rand: () => number = Math.random): number {
  return RETRY_BACKOFF_MIN_MS + rand() * (RETRY_BACKOFF_MAX_MS - RETRY_BACKOFF_MIN_MS)
}
