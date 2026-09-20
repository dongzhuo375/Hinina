/**
 * 设置入口的「隐藏解锁」：设置按钮默认不可见，**连点底部版本号 5 下**才出现。
 *
 * 为什么这样设计：设置页里是可改变客户端行为的开关（OJ 地址、轮询节拍、缓存），
 * 赛场上的选手误触后很难自查（改了服务器地址就再也连不上）。把它藏进一个需要
 * 主动寻找的入口，等于给「我不该点这个」加了一道最低成本的闸门。
 *
 * 状态**仅本次运行有效、不落盘**：解锁是「我现在要调试」的临时意图，不是配置。
 * 重启后重新隐藏 —— 否则一次误触会永久暴露设置入口，闸门形同虚设。
 *
 * 判定逻辑抽成注入式状态机（`createUnlockGate`，时间源可注入）以便穷尽单测：
 * 「间隔超窗重新计数」「连点满 N 次解锁」「解锁后不再计数」这些边界靠真实点击
 * 测不出来。
 */

import { ref } from 'vue'

/** 解锁所需的连点次数。 */
export const SETTINGS_UNLOCK_TAPS = 5

/**
 * 相邻两次点击的最大间隔（毫秒）。
 *
 * 超过即**重新从 1 计数**（而不是继续累加）：否则「一天里零散点 5 次」也会解锁，
 * 闸门就失去意义。2 秒是「连点」的自然节奏上限。
 */
export const SETTINGS_TAP_WINDOW_MS = 2_000

export interface UnlockGate {
  /** 记录一次点击；返回**本次之后**是否已解锁。 */
  tap(): boolean
  /** 当前是否已解锁。 */
  unlocked(): boolean
}

export interface UnlockGateOptions {
  /** 解锁所需连点次数（默认 [`SETTINGS_UNLOCK_TAPS`]） */
  taps?: number
  /** 相邻点击的最大间隔毫秒（默认 [`SETTINGS_TAP_WINDOW_MS`]） */
  windowMs?: number
  /** 时间源（默认 `Date.now`；测试注入以驱动窗口边界） */
  now?: () => number
}

/**
 * 创建解锁闸门（纯状态机，无 Vue 依赖）。
 *
 * 计数规则：间隔在窗口内则累加，超窗则从 1 重新开始；达到 `taps` 次即解锁。
 * 解锁后 `tap()` 直接返回 `true` 且不再计数（幂等，重复点击无副作用）。
 */
export function createUnlockGate(options: UnlockGateOptions = {}): UnlockGate {
  const taps = options.taps ?? SETTINGS_UNLOCK_TAPS
  const windowMs = options.windowMs ?? SETTINGS_TAP_WINDOW_MS
  const now = options.now ?? (() => Date.now())

  let count = 0
  let lastTapAt = 0
  let isUnlocked = false

  return {
    tap(): boolean {
      if (isUnlocked) return true

      const at = now()
      // 首次点击时 lastTapAt 为 0，差值必然超窗 → 计数为 1，与「从 1 开始」一致
      count = at - lastTapAt <= windowMs ? count + 1 : 1
      lastTapAt = at

      if (count >= taps) {
        count = 0
        isUnlocked = true
      }
      return isUnlocked
    },
    unlocked(): boolean {
      return isUnlocked
    },
  }
}

// ── 应用级单例 ──
//
// 两个兄弟组件需要同一份状态（`StatusBar` 产生点击、`ActivityBar` 决定是否渲染
// 设置项）。这里刻意**不引入 Pinia store**：它没有领域数据、不走 IPC、不参与
// 数据流分层，一个会话级布尔量不值得为它多一个 store 文件。

const gate = createUnlockGate()

/** 设置入口是否已解锁（响应式；供模板直接消费）。 */
export const settingsUnlocked = ref(false)

/**
 * 记录一次版本号点击。
 *
 * @returns 本次点击是否**刚刚**解锁（`true` 仅出现一次，供调用方给出一次性反馈）
 */
export function tapVersion(): boolean {
  const wasUnlocked = gate.unlocked()
  gate.tap()
  settingsUnlocked.value = gate.unlocked()
  return !wasUnlocked && settingsUnlocked.value
}
