/**
 * 可复用的轮询原语（榜单 10s±2s、题目总览 30s±5s 等场景共用）。
 *
 * 设计要点：
 * - **抖动错峰**：全场客户端往往在同一时刻进入视图（如开赛瞬间），若轮询间隔固定，
 *   各客户端的请求相位将永久保持同步，每个周期都在同一毫秒打到 OJ 服务器上形成
 *   周期性尖峰。给间隔加上均匀分布的 ±jitter 后，每个周期的相位被随机打散，
 *   尖峰被摊平成均匀流量（HOJ 榜单文档 §9.9 也要求轮询间隔 ≥10s 并在后台暂停）。
 * - **递归 setTimeout 而非 setInterval**：setInterval 的间隔在创建时固定，
 *   无法让抖动逐周期生效；每次 tick 后重新计算下一周期延迟才能持续错峰。
 * - **可测性**：随机源与定时器函数全部注入，测试无需 fake timers 也能精确推进时间。
 */

/**
 * 计算一次轮询延迟：`intervalMs ± jitterMs`，抖动在区间内均匀分布。
 *
 * 结果恒非负（jitter > interval 时下界被钳到 0），避免负延迟导致
 * setTimeout 立即触发、退化为忙轮询。
 */
export function planPollDelayMs(
  intervalMs: number,
  jitterMs: number,
  rand: () => number = Math.random,
): number {
  const jitter = (rand() * 2 - 1) * jitterMs
  return Math.max(0, intervalMs + jitter)
}

export interface PollerOptions {
  /** 每周期执行的任务；抛错/拒绝不会终止轮询（内部捕获并交给 onError） */
  task: () => void | Promise<void>
  intervalMs: number
  jitterMs?: number
  /** 返回 true 时跳过本次 task（如页面隐藏），但仍排下一周期 */
  isPaused?: () => boolean
  onError?: (e: unknown) => void
  rand?: () => number
  setTimeoutFn?: (handler: () => void, ms: number) => ReturnType<typeof setTimeout>
  clearTimeoutFn?: (handle: ReturnType<typeof setTimeout>) => void
}

export interface Poller {
  /** 启动轮询（幂等：重复调用不会产生多个定时器）；首次 task 在一个完整延迟后执行 */
  start(): void
  /** 停止轮询并清理定时器句柄（幂等） */
  stop(): void
  isRunning(): boolean
}

/**
 * 创建递归 setTimeout 轮询器。
 *
 * 不变量：
 * - 任意时刻至多存在一个待触发的定时器句柄；
 * - **重入保护**：上一次 task（含异步）未结束时到点，则跳过本次执行、只续排下一周期，
 *   绝不叠加并发请求（慢网络下请求堆积会拖垮 OJ 服务端与本地渲染）；
 * - 句柄保存在闭包普通变量中，调用方**不得**把它放进 ref/reactive 等响应式系统
 *   （Vue 深层代理定时器句柄既无意义又可能干扰宿主环境的句柄语义）。
 */
export function createPoller(options: PollerOptions): Poller {
  const {
    task,
    intervalMs,
    jitterMs = 0,
    isPaused,
    onError,
    rand = Math.random,
    setTimeoutFn = (handler, ms) => setTimeout(handler, ms),
    clearTimeoutFn = (handle) => clearTimeout(handle),
  } = options

  let handle: ReturnType<typeof setTimeout> | null = null
  let running = false
  /// task 正在执行（同步未完成或 Promise 未 settle）——重入保护标志
  let busy = false

  const scheduleNext = (): void => {
    handle = setTimeoutFn(tick, planPollDelayMs(intervalMs, jitterMs, rand))
  }

  const runTask = (): void => {
    busy = true
    const settle = (): void => {
      busy = false
    }
    const fail = (e: unknown): void => {
      busy = false
      onError?.(e)
    }
    try {
      const result = task()
      if (result instanceof Promise) {
        result.then(settle, fail)
      } else {
        settle()
      }
    } catch (e) {
      fail(e)
    }
  }

  const tick = (): void => {
    handle = null
    if (!running) return
    // 先续排下一周期：暂停/重入只跳过本次 task，不打断轮询节奏
    scheduleNext()
    if (busy) return
    if (isPaused?.()) return
    runTask()
  }

  return {
    start(): void {
      if (running) return
      running = true
      scheduleNext()
    },
    stop(): void {
      running = false
      if (handle !== null) {
        clearTimeoutFn(handle)
        handle = null
      }
    },
    isRunning(): boolean {
      return running
    },
  }
}
