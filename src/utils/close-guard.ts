/**
 * 关窗守卫：退出前把在途编辑落盘，且**保证窗口最终一定能关上**。
 *
 * 为什么需要它：编辑器改动只在防抖后到达后端内存，auto-save 周期最长 30 秒 ——
 * 直接关窗会丢掉这段窗口内的编辑（数据丢失，选手往往到重新打开才发现）。
 *
 * 为什么单独成模块：原实现内联在 `main.ts` 的 `onCloseRequested` 回调里，有两个
 * 致命的失效模式，且因为依赖真实窗口而完全无法测试：
 *
 * 1. **一旦失败就永久卡死**：原实现用 `flushing` 布尔量表示「落盘在途」，落盘结束
 *    后调用 `getCurrentWindow().close()` 收尾。若这次 `close()` 抛错（权限、IPC
 *    竞态）而异常被 `void` 吞掉，`flushing` 就永久停在 `true` —— 此后**每一次**点
 *    关闭按钮都命中 `if (flushing) return`，在 `preventDefault()` 之后直接返回，
 *    窗口再也关不掉。这是「窗口无法关闭」的直接原因。
 * 2. **收尾依赖二次 close-requested**：在 `close-requested` 回调里再调 `close()`
 *    会重新触发一次 `close-requested`，靠 `proceedClose` 标志放行。窗口此时已处于
 *    closing 状态，该事件可能根本不再投递，关闭请求被静默丢弃。
 *
 * 本实现的状态机：
 * ```
 *  idle ──handleRequest()──▶ flushing ──落盘完成/超时──▶ allowing ──▶ destroy()
 *    ▲                          │
 *    └──── 直接放行（返回 false）─┘  ← allowing 状态下再来的请求
 * ```
 * - `idle` 收到请求：拦截（返回 `true`），开始落盘；
 * - `flushing` 收到请求：**仍然拦截**（返回 `true`）但不重复落盘 —— 不耐烦的双击
 *   不该中断在途落盘（等于零超时丢数据）；
 * - `allowing` 收到请求：放行（返回 `false`）。
 *
 * 收尾用 `destroy()` 而不是再调一次 `close()`：落盘已经完成，我们**不需要**再走
 * 一遍 close-requested 往返，而那个往返正是失效模式 2。`destroy()` 直接销毁窗口，
 * 不会被本守卫再次拦截。`destroy()` 失败时才退回 `close()` 兜底。
 *
 * 双层时间上界保证「无论如何都能关上」：
 * - `flushTimeoutMs`：落盘最多等这么久，超时即进入收尾；
 * - `hardTimeoutMs`：从拦截那一刻起算的绝对上界，到点无论处于什么状态都强制收尾
 *   （兜住「落盘 Promise 永不 settle」与「收尾本身卡住」两种极端）。
 */

/** 落盘等待上限（毫秒）：超时即放行关闭，避免 IPC 无响应时窗口关不掉 */
export const DEFAULT_FLUSH_TIMEOUT_MS = 3_000

/** 硬超时（毫秒）：从拦截那一刻起算，到点无论如何都强制销毁窗口 */
export const DEFAULT_HARD_TIMEOUT_MS = 5_000

/** 守卫状态 */
type CloseGuardState = 'idle' | 'flushing' | 'allowing'

export interface CloseGuardOptions {
  /** 关窗前的落盘动作（内部会先把在途改动推送到后端再落盘） */
  flush: () => Promise<unknown>
  /** 常规关闭窗口（会再次触发 close-requested） */
  close: () => Promise<unknown>
  /** 强制销毁窗口（绕过 close-requested，不会被本守卫再次拦截） */
  destroy: () => Promise<unknown>
  flushTimeoutMs?: number
  hardTimeoutMs?: number
  /** 告警出口（默认 `console.error`；测试可注入以断言降级路径） */
  onWarn?: (message: string, error?: unknown) => void
  setTimeoutFn?: (handler: () => void, ms: number) => ReturnType<typeof setTimeout>
  clearTimeoutFn?: (handle: ReturnType<typeof setTimeout>) => void
}

export interface CloseGuard {
  /**
   * 处理一次关窗请求（**同步**返回，供 `event.preventDefault()` 直接使用）。
   *
   * @returns `true` = 本次已拦截，守卫会自行完成落盘并关闭窗口；
   *          `false` = 放行，调用方不应 `preventDefault()`。
   */
  handleRequest(): boolean
  /** 当前状态（测试与诊断用） */
  state(): CloseGuardState
}

/**
 * 创建关窗守卫。
 *
 * 依赖全部注入（`flush` / `close` / `destroy` / 定时器），因此不需要真实窗口即可
 * 穷尽测试「落盘成功」「落盘抛错」「落盘永不 settle」「close 失败」「重复点击」
 * 等路径 —— 这些正是原实现出问题的地方。
 */
export function createCloseGuard(options: CloseGuardOptions): CloseGuard {
  const {
    flush,
    close,
    destroy,
    flushTimeoutMs = DEFAULT_FLUSH_TIMEOUT_MS,
    hardTimeoutMs = DEFAULT_HARD_TIMEOUT_MS,
    onWarn = (message: string, error?: unknown) => console.error(message, error),
    setTimeoutFn = (handler, ms) => setTimeout(handler, ms),
    clearTimeoutFn = (handle) => clearTimeout(handle),
  } = options

  let currentState: CloseGuardState = 'idle'

  const delay = (ms: number): Promise<void> =>
    new Promise((resolve) => {
      setTimeoutFn(resolve, ms)
    })

  /**
   * 收尾：进入 `allowing` 并销毁窗口。
   *
   * 幂等 —— 落盘完成与硬超时可能同时到达，只有第一个生效。
   */
  const finalize = async (): Promise<void> => {
    if (currentState === 'allowing') return
    currentState = 'allowing'
    try {
      await destroy()
    } catch (destroyError) {
      // destroy 不可用（权限缺失 / 平台差异）时退回常规 close：
      // 此时状态已是 allowing，close-requested 会被放行
      onWarn('销毁窗口失败，退回常规关闭:', destroyError)
      try {
        await close()
      } catch (closeError) {
        onWarn('关闭窗口失败（窗口可能仍处于打开状态）:', closeError)
      }
    }
  }

  const runFlush = async (): Promise<void> => {
    // 绝对上界：落盘 Promise 永不 settle 时也要能关上窗口
    const hardTimer = setTimeoutFn(() => {
      if (currentState !== 'flushing') return
      onWarn('关窗落盘超过硬超时，强制关闭窗口')
      void finalize()
    }, hardTimeoutMs)

    try {
      // 落盘失败不阻断退出：卡住窗口比丢一次自动备份更糟
      // （内容仍留在后端内存，且下次编辑会重新落盘）
      await Promise.race([flush().then(() => undefined, () => undefined), delay(flushTimeoutMs)])
    } finally {
      clearTimeoutFn(hardTimer)
    }
    await finalize()
  }

  return {
    handleRequest(): boolean {
      if (currentState === 'allowing') return false
      if (currentState === 'flushing') return true
      currentState = 'flushing'
      void runFlush()
      return true
    },
    state(): CloseGuardState {
      return currentState
    },
  }
}
