import type { App } from 'vue'
import type { Router } from 'vue-router'
import type { FrontendLogLevel } from '@/bridge/diagnostics.bridge'
import type { IpcError } from '@/bridge'

/// 全局错误转发函数：由组合根注入（桥到 Rust `frontend_log`）。
export type FrontendLogForward = (level: FrontendLogLevel, scope: string, message: string) => void

/// 与 Rust 端 `MAX_MESSAGE_CHARS` 一致的长度上限 —— 前端先截一次，避免把超大
/// payload（如被序列化进错误消息的对象）整段送过 IPC。
const MESSAGE_MAX_CHARS = 2000

/// 限频窗口：同签名错误 1 秒内只转发一次。渲染循环式的重复错误（setup 里同步
/// throw → Vue 重渲染 → 再 throw）否则会以每帧数条的密度打爆日志文件。
const DEDUP_WINDOW_MS = 1000

/// 去重表容量上限：超出即整表清空（错误种类爆表时宁可漏记也不无界增长）。
const DEDUP_TABLE_MAX = 256

/**
 * 安装全局错误转发 —— 仅应在组合根（`main.ts`）调用一次，且**必须先于 mount**：
 * 首屏渲染/setup 抛出的错误只有 errorHandler 已在位才会被捕获。
 *
 * 覆盖四类「JS 世界的崩溃」，它们此前只进 console（用户不开 DevTools 就无痕，
 * Rust 日志看不到）—— panic 钩子的前端半边：
 * - `app.config.errorHandler`：组件渲染 / 生命周期 / watcher 内的错误；
 * - `window` `error` 事件：未捕获的同步异常与资源加载失败；
 * - `unhandledrejection`：无人接的 Promise 拒绝（IpcError 归一为 `ipc:{cmd}`）；
 * - `router.onError`：异步组件加载失败 / 导航守卫抛错。
 *
 * 设计约束：
 * - **守卫自身绝不抛错**：所有钩子体包 try/catch；`forward` 抛错同样被吞
 *   （报错的通道不允许自己成为新的错误源）。
 * - **限频去重**：同签名（scope + 消息前 200 字符）1 秒内只转发一次。
 * - **不 `preventDefault`** unhandledrejection：默认的 console 输出保留，
 *   DevTools 现场排障不受影响；本守卫是补一份持久化，不是接管输出。
 * - 依赖倒置同 `sessionGuard`：本模块不感知 bridge 实现与 Rust，转发函数注入。
 *
 * 返回清理函数（移除监听、恢复默认 errorHandler）—— 仅供测试拆卸；组合根
 * 不拆卸（与应用同生命周期）。
 */
export function installGlobalErrorGuard(
  app: App,
  router: Router,
  forward: FrontendLogForward,
): () => void {
  const lastSeen = new Map<string, number>()

  /// 核心上报：归一 → 去重 → 转发。任何异常都吞掉（守卫不许成为错误源）。
  function report(scope: string, error: unknown, detail?: string): void {
    try {
      const message = normalize(error, detail)
      const key = `${scope}|${message.slice(0, 200)}`
      const now = Date.now()
      const seen = lastSeen.get(key)
      if (seen !== undefined && now - seen < DEDUP_WINDOW_MS) return
      // 容量护栏：去重表有界（错误种类异常爆发时整表清空，宁可漏记不无界增长）
      if (lastSeen.size >= DEDUP_TABLE_MAX) lastSeen.clear()
      lastSeen.set(key, now)
      try {
        forward('error', scope, message)
      } catch {
        // 同步抛错 = 这条没送出去：撤销去重记录，同签名错误在窗口内可重试
        //（异步拒绝由组合根的 catch 吞掉，这里看不到，窗口内不重试 —— 尽力而为）
        lastSeen.delete(key)
      }
    } catch {
      // 吞掉：转发失败（IPC 不可用等）不追责到错误源本身
    }
  }

  // Vue 组件树内错误（渲染 / 生命周期 / watcher）。info 是 Vue 给出的出错环节
  // （如 "render function"），与消息一起落盘便于归因。
  app.config.errorHandler = (err, _instance, info) => {
    report('vue', err, `vue:${info}`)
  }

  // 未捕获同步异常 + 资源加载失败。后者没有 error 对象，从事件目标的 src/href 取址
  //（本地静态资源名，非敏感）。
  const onWindowError = (event: ErrorEvent): void => {
    const source = event.error ?? event.message
    if (source) {
      report('window', source)
      return
    }
    const target = event.target
    if (target && target !== window) {
      const asset = (target as { src?: string; href?: string }).src
        ?? (target as { href?: string }).href
        ?? 'unknown'
      report('window', `资源加载失败: ${asset}`)
    }
  }
  window.addEventListener('error', onWindowError)

  // 无人接的 Promise 拒绝。IpcError 鸭子类型识别（cmd + message）→ `ipc:{cmd}` 前缀，
  // 与 bridge 的单点日志互相印证。
  const onUnhandledRejection = (event: PromiseRejectionEvent): void => {
    report('promise', event.reason)
  }
  window.addEventListener('unhandledrejection', onUnhandledRejection)

  // 异步组件 chunk 加载失败 / 导航守卫抛错（此前只有 console，无任何留痕）。
  const offRouterError = router.onError((error) => {
    report('router', error)
  })

  return () => {
    window.removeEventListener('error', onWindowError)
    window.removeEventListener('unhandledrejection', onUnhandledRejection)
    offRouterError()
    app.config.errorHandler = undefined
    lastSeen.clear()
  }
}

/// 错误归一：Error → message（附 detail 环节）；IpcError 鸭子识别 → `ipc:{cmd}`；
/// 空/null → 占位；其余 String 化。统一截断到与 Rust 端一致的上限（Rust 侧还会
/// 再兜底一次）。
function normalize(error: unknown, detail?: string): string {
  let message: string
  if (error instanceof Error) {
    message = error.message
  } else if (
    error &&
    typeof error === 'object' &&
    typeof (error as IpcError).cmd === 'string' &&
    typeof (error as IpcError).message === 'string'
  ) {
    const ipc = error as IpcError
    message = `ipc:${ipc.cmd}: ${ipc.message}`
  } else if (typeof error === 'string' && error.length > 0) {
    message = error
  } else if (error === null || error === undefined) {
    // undefined reason 是真实世界最常出现的拒绝形态（Promise 链里裸 reject()）
    message = '<empty error>'
  } else {
    try {
      message = JSON.stringify(error) ?? String(error)
    } catch {
      message = String(error)
    }
  }
  if (message.length === 0) message = '<empty error>'
  const suffix = detail ? ` (${detail})` : ''
  const room = MESSAGE_MAX_CHARS - suffix.length
  const body = message.length > room ? `${message.slice(0, room)}…` : message
  return `${body}${suffix}`
}
