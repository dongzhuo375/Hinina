import { beforeEach, describe, expect, it } from 'vitest'
import {
  createUnlockGate,
  SETTINGS_TAP_WINDOW_MS,
  SETTINGS_UNLOCK_TAPS,
  settingsUnlocked,
  tapVersion,
} from '@/utils/settings-access'

/// 可控时间源：把「连点窗口」的边界变成可断言的整数，而不是靠真实等待
function fakeClock(start = 1_000_000) {
  let current = start
  return {
    now: () => current,
    advance: (ms: number) => {
      current += ms
    },
  }
}

describe('解锁闸门 — 计数规则', () => {
  it('默认需要连点 5 下，第 5 下才解锁', () => {
    const clock = fakeClock()
    const gate = createUnlockGate({ now: clock.now })

    for (let i = 1; i < SETTINGS_UNLOCK_TAPS; i++) {
      expect(gate.tap()).toBe(false)
      expect(gate.unlocked()).toBe(false)
      clock.advance(100)
    }

    expect(gate.tap()).toBe(true)
    expect(gate.unlocked()).toBe(true)
  })

  it('间隔超窗即从 1 重新计数（零散点击不能累加成解锁）', () => {
    const clock = fakeClock()
    const gate = createUnlockGate({ now: clock.now })

    // 点 4 下（差一次就解锁），然后每次都等到超窗
    for (let i = 0; i < 4; i++) {
      expect(gate.tap()).toBe(false)
      clock.advance(SETTINGS_TAP_WINDOW_MS + 1)
    }

    // 再点一次：计数回到 1 而不是凑满 5
    expect(gate.tap()).toBe(false)
    expect(gate.unlocked()).toBe(false)
  })

  it('窗口边界取「闭区间」：恰好等于窗口仍算连点', () => {
    const clock = fakeClock()
    const gate = createUnlockGate({ taps: 2, windowMs: 1_000, now: clock.now })

    expect(gate.tap()).toBe(false)
    clock.advance(1_000)
    expect(gate.tap()).toBe(true)
  })

  it('解锁后重复点击幂等（不重复计数、不重复报解锁）', () => {
    const clock = fakeClock()
    const gate = createUnlockGate({ taps: 2, now: clock.now })

    gate.tap()
    clock.advance(50)
    expect(gate.tap()).toBe(true)
    // 解锁后即使间隔超窗也保持解锁
    clock.advance(SETTINGS_TAP_WINDOW_MS * 10)
    expect(gate.tap()).toBe(true)
    expect(gate.unlocked()).toBe(true)
  })

  it('可注入自定义次数与窗口', () => {
    const clock = fakeClock()
    const gate = createUnlockGate({ taps: 3, windowMs: 500, now: clock.now })

    expect(gate.tap()).toBe(false)
    clock.advance(500)
    expect(gate.tap()).toBe(false)
    clock.advance(500)
    expect(gate.tap()).toBe(true)
  })
})

describe('应用级单例', () => {
  beforeEach(() => {
    settingsUnlocked.value = false
  })

  it('tapVersion 只在「刚刚解锁」那一次返回 true', () => {
    const results = Array.from({ length: SETTINGS_UNLOCK_TAPS }, () => tapVersion())

    expect(results).toEqual([false, false, false, false, true])
    expect(settingsUnlocked.value).toBe(true)
    // 解锁后再点：状态保持，但不再报「刚刚解锁」
    expect(tapVersion()).toBe(false)
    expect(settingsUnlocked.value).toBe(true)
  })
})
