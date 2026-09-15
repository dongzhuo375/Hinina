import { describe, expect, it } from 'vitest'
import {
  IMMEDIATE_JITTER_MS,
  PERIODIC_JITTER_MS,
  PERIODIC_RECHECK_MS,
  PRECHECK_STOP_MS,
  PRECHECK_WINDOW_END_MS,
  PRECHECK_WINDOW_START_MS,
  RETRY_BACKOFF_MAX_MS,
  RETRY_BACKOFF_MIN_MS,
  planNextPrecheck,
  planRetryDelayMs,
} from '@/utils/session-check'
import type { PrecheckPlan } from '@/utils/session-check'

/// 固定开赛时刻，测试只关心"距开赛多久"
const START = 10_000_000_000

/// 距开赛 untilStart 毫秒时的排程
function planAt(untilStart: number, rand: () => number): PrecheckPlan | null {
  return planNextPrecheck(START - untilStart, START, rand)
}

/// 排程的实际执行时刻
function fireAt(untilStart: number, plan: PrecheckPlan): number {
  return START - untilStart + plan.delayMs
}

const rand0 = () => 0
const randHalf = () => 0.5
/// Math.random 取不到 1，此处用 1 验证数学上界（真实延迟严格小于该值）
const rand1 = () => 1

const HOUR = 60 * 60 * 1000
const MIN = 60 * 1000

describe('planNextPrecheck — 停止条件', () => {
  it('距开赛不足 30s 时不再预检（交给进场流程与全局 401 兜底）', () => {
    expect(planAt(PRECHECK_STOP_MS, randHalf)).toBeNull()
    expect(planAt(PRECHECK_STOP_MS - 1, randHalf)).toBeNull()
  })

  it('已开赛/已过开赛时刻不再预检', () => {
    expect(planAt(0, randHalf)).toBeNull()
    expect(planAt(-MIN, randHalf)).toBeNull()
  })
})

describe('planNextPrecheck — 周期复检（距开赛 > 10min）', () => {
  it('rand=0.5 时无抖动，间隔为 5min', () => {
    expect(planAt(HOUR, randHalf)).toEqual({
      delayMs: PERIODIC_RECHECK_MS,
      reason: 'periodic',
    })
  })

  it('抖动范围为 ±60s（打散同一批开机的客户端）', () => {
    expect(planAt(HOUR, rand0)?.delayMs).toBe(PERIODIC_RECHECK_MS - PERIODIC_JITTER_MS)
    expect(planAt(HOUR, rand1)?.delayMs).toBe(PERIODIC_RECHECK_MS + PERIODIC_JITTER_MS)
  })

  it('不会越过窗口左边界：到点后重新规划为窗口内随机取点', () => {
    // 距开赛 11min，距窗口左边界仅 1min → 截断为 1min 而非 5min
    const untilStart = PRECHECK_WINDOW_START_MS + MIN
    expect(planAt(untilStart, randHalf)).toEqual({ delayMs: MIN, reason: 'periodic' })
  })
})

describe('planNextPrecheck — 窗口内一次性错峰（T-10min ~ T-3min）', () => {
  it('窗口左边界（恰好 10min）归入窗口分支', () => {
    expect(planAt(PRECHECK_WINDOW_START_MS, randHalf)?.reason).toBe('window')
  })

  it('在剩余窗口区间内随机取点', () => {
    const untilStart = 6 * MIN // 距开赛 6min → 剩余窗口 3min
    const span = untilStart - PRECHECK_WINDOW_END_MS
    expect(planAt(untilStart, rand0)).toEqual({ delayMs: 0, reason: 'window' })
    expect(planAt(untilStart, rand1)).toEqual({ delayMs: span, reason: 'window' })
    expect(planAt(untilStart, randHalf)?.delayMs).toBe(span / 2)
  })

  it('预检点不晚于 T-3min —— 为"发现失效 → 重新登录"留出缓冲', () => {
    const untilStart = 6 * MIN
    const plan = planAt(untilStart, rand1)!
    expect(fireAt(untilStart, plan)).toBe(START - PRECHECK_WINDOW_END_MS)
  })
})

describe('planNextPrecheck — 迟到启动（距开赛 ≤ 3min）', () => {
  it('窗口右边界归入立即执行分支', () => {
    expect(planAt(PRECHECK_WINDOW_END_MS, randHalf)?.reason).toBe('immediate')
  })

  it('0–3s 抖动后立即执行（避免同批迟到客户端齐发）', () => {
    expect(planAt(MIN, rand0)).toEqual({ delayMs: 0, reason: 'immediate' })
    expect(planAt(MIN, rand1)?.delayMs).toBe(IMMEDIATE_JITTER_MS)
  })
})

describe('planNextPrecheck — 全局不变量', () => {
  it('任意时刻、任意随机值下：延迟非负且预检不会排到开赛之后', () => {
    const rands = [rand0, randHalf, rand1]
    for (let untilStart = PRECHECK_STOP_MS + 1; untilStart <= 3 * HOUR; untilStart += 7_919) {
      for (const rand of rands) {
        const plan = planNextPrecheck(START - untilStart, START, rand)
        expect(plan, `untilStart=${untilStart} 应仍有排程`).not.toBeNull()
        expect(plan!.delayMs, `untilStart=${untilStart} 延迟不应为负`).toBeGreaterThanOrEqual(0)
        expect(
          START - untilStart + plan!.delayMs,
          `untilStart=${untilStart} 预检不应晚于开赛`,
        ).toBeLessThan(START)
      }
    }
  })
})

describe('planRetryDelayMs', () => {
  it('退避区间为 2–5s（只重试一次，避免重试风暴）', () => {
    expect(planRetryDelayMs(rand0)).toBe(RETRY_BACKOFF_MIN_MS)
    expect(planRetryDelayMs(rand1)).toBe(RETRY_BACKOFF_MAX_MS)
    expect(planRetryDelayMs(randHalf)).toBe((RETRY_BACKOFF_MIN_MS + RETRY_BACKOFF_MAX_MS) / 2)
  })
})
