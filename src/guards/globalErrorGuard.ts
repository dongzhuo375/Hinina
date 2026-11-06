import type { App } from 'vue'
import type { Router } from 'vue-router'
import type { FrontendLogLevel } from '@/bridge/diagnostics.bridge'
import { IpcError } from '@/bridge'

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
 * 覆盖五类「JS 世界的崩溃 / 策略违规」，它们此前只进 console（用户不开 DevTools
 * 就无痕，Rust 日志看不到）—— panic 钩子的前端半边：
 * - `app.config.errorHandler`：组件渲染 / 生命周期 / watcher 内的错误；
 * - `window` `error` 事件：未捕获的同步异常与资源加载失败；
 * - `unhandledrejection`：无人接的 Promise 拒绝（IpcError 归一为 `ipc:{cmd}`）；
 * - `router.onError`：异步组件加载失败 / 导航守卫抛错；
 * - `document` `securitypolicyviolation`：CSP 违规 —— 资源类违规（img/media/
 *   font/worker…被策略拦截）**不抛 JS 异常**，但被拦元素**同样触发上面的
 *   window error 钩子**（这正是「双报」来源：error 行带文件名、violation 行带
 *   指令与来源宿主）；无元素关联的违规（如 JS `fetch` 被 connect-src 拦）则只
 *   走本钩子。生产端（exe）CSP 配置漂移此前零可观测性。
 *
 * 设计约束：
 * - **守卫自身绝不抛错**：所有钩子体包 try/catch；`forward` 抛错同样被吞
 *   （报错的通道不允许自己成为新的错误源）。
 * - **限频去重**：同签名（scope + 完整归一化消息，截前缀会误并同前缀不同后缀的错误）1 秒内只转发一次。
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
      // 去重键用**完整归一化消息**：截前缀会把「同前缀 + 变化后缀」的不同错误
      // 误并成一条（窗口内后到的直接丢失）。message 已截断至 2000 字符，键有界。
      const key = `${scope}|${message}`
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
  // **必须补 console.error**：设置了 errorHandler 后 Vue 不再走默认输出，不补
  // 这一行，DevTools 里组件错误就消失（转发是持久化，console 是现场排障）。
  app.config.errorHandler = (err, _instance, info) => {
    console.error(err, info)
    report('vue', err, `vue:${info}`)
  }

  // 未捕获同步异常 + 资源加载失败。
  const onWindowError = (event: ErrorEvent): void => {
    const source = event.error ?? event.message
    if (source) {
      report('window', source)
      return
    }
    const target = event.target
    if (target && target !== window) {
      report('window', resourceFailureMessage(target))
    }
  }

  /// 资源加载失败消息：**同源**记标签名与路径末段文件名（本地资产诊断——
  /// 同源文件数有界）；**跨源收敛到 host 级**（与 CSP 违规行同一预算考量：
  /// 题面外链资源成批失败/被拦时按文件名一行一个会刷满 Rust 端 60s 滚动预
  /// 算、挤掉同窗口其它错误）；data: 只落协议+MIME。**完整 URL 绝不进日志**
  /// ——外链可能带签名令牌查询参数（URL 解析天然分离查询串与 fragment，
  /// 相对地址按基址解析后同样走该口径，无字符串切分分支可泄漏）。
  function resourceFailureMessage(target: EventTarget): string {
    const tag = (target as { tagName?: unknown }).tagName
    const name = typeof tag === 'string' ? tag.toLowerCase() : 'unknown'
    const raw =
      (target as { src?: unknown }).src ?? (target as { href?: unknown }).href
    return `资源加载失败: <${name}> ${resourceSourceLabel(raw)}`
  }

  /// 资源地址标签：同源 → 文件名；跨源 → host；data: → 协议+MIME；其余
  /// （含解析后落在基址根路径的相对地址）→ unknown。
  function resourceSourceLabel(raw: unknown): string {
    if (typeof raw !== 'string' || raw.length === 0) return 'unknown'
    if (raw.startsWith('data:')) return dataUriLabel(raw)
    try {
      const url = new URL(raw, window.location.href)
      return url.host === window.location.host
        ? url.pathname.split('/').pop() || 'unknown'
        : url.host
    } catch {
      return 'unknown'
    }
  }

  // **必须 capture 阶段**：资源加载失败的 error 事件派发在元素上且**不冒泡**
  //（HTML 规范），bubble 阶段的 window 监听永远收不到 —— 只有 capture 沿
  // window → document → … → target 的传播路径能拦住它。未捕获异常的 error
  // 事件派发在 window 自身（target 阶段），不受此参数影响。
  window.addEventListener('error', onWindowError, true)

  // 无人接的 Promise 拒绝。IpcError（ipcInvoke 的统一错误类型）归一为
  // `ipc:{cmd}` 前缀 —— 哪个命令失败是排障的第一线索，与 bridge 的单点日志互相印证。
  const onUnhandledRejection = (event: PromiseRejectionEvent): void => {
    report('promise', event.reason)
  }
  window.addEventListener('unhandledrejection', onUnhandledRejection)

  // 异步组件 chunk 加载失败 / 导航守卫抛错（此前只有 console，无任何留痕）。
  const offRouterError = router.onError((error) => {
    report('router', error)
  })

  // CSP 违规：见文件头第五条。挂 document 是权威建议（MDN）：事件**冒泡至
  // Window 且 composed**，Chromium 对资源类违规（被拦 <img> 等）**直接以
  // document 为 target 派发**、不经被拦元素冒泡——capture 监听在「直接派发」
  // 与「元素冒泡」两种模型下都收得到，属双保险而非必需。来源标签收敛到
  // **host 级**（cspSourceLabel）：被拦资源与 window error 行双报，error 行已
  // 带文件名，本行只需指令 + 来源宿主——同 host 同指令经限频去重合并，防大
  // 批被拦资源刷满 Rust 端 frontend_log 的 60s 滚动预算、挤掉同窗口的其它错
  // 误。浏览器自带的 console 报文保留（转发是持久化，不是接管输出），不重复
  // 补 console。
  const onCspViolation = (event: Event): void => {
    const e = event as SecurityPolicyViolationEvent
    // TS lib 只类型化标准的 effectiveDirective（Chrome 运行时另有非标准的
    // directive 同值字段，不依赖）；两个都缺的怪异载荷落 'unknown'
    const directive = e.effectiveDirective || 'unknown'
    const disposition = typeof e.disposition === 'string' ? e.disposition : ''
    report('csp', `CSP ${directive} 被拦 ${disposition} ${cspSourceLabel(e.blockedURI)}`.replace(/\s+/g, ' ').trim())
  }
  document.addEventListener('securitypolicyviolation', onCspViolation, true)

  return () => {
    // removeEventListener 按 type+listener+capture 匹配：必须与注册时的
    // capture: true 一致才能真的移除
    window.removeEventListener('error', onWindowError, true)
    window.removeEventListener('unhandledrejection', onUnhandledRejection)
    document.removeEventListener('securitypolicyviolation', onCspViolation, true)
    offRouterError()
    app.config.errorHandler = undefined
    lastSeen.clear()
  }
}

/// 错误归一：**IpcError 先于 Error 判定**（IpcError extends Error，只按
/// instanceof Error 走会丢失 `ipc:{cmd}` 前缀 —— 归一化曾是死代码，评审抓出）
/// → message + 截断后的 stack（Rust 端为保留换行/制表所做的处理因此真正生效）；
/// 空/null → 占位；其余 String 化。统一按**码点**截断到与 Rust 端一致的上限
/// （Rust 侧还会再兜底一次）。
function normalize(error: unknown, detail?: string): string {
  let message: string
  if (error instanceof IpcError) {
    message = `ipc:${error.cmd}: ${error.message}`
  } else if (error instanceof Error) {
    message = messageWithStack(error)
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
  // -1 为省略号预留：truncateByChars 截断时会追加一个字符，不预留则总长
  // 变成 2001，Rust 端再截一次会把后缀末字符切掉并追加第二个省略号
  //（长栈错误 + 环节后缀必然命中，评审复现 len=2001）
  const room = MESSAGE_MAX_CHARS - suffix.length - 1
  const body = truncateByChars(message, room)
  return `${body}${suffix}`
}

/// Error → message + stack。V8 的 stack 首行自带消息（去重）；SpiderMonkey/
/// JavaScriptCore 的 stack 不含消息头（补一行）。栈只取头部若干帧 —— 截断由
/// 2000 字符上限自然完成，足够定位又不会把日志撑成单条巨行。
function messageWithStack(error: Error): string {
  const stack = typeof error.stack === 'string' ? error.stack : undefined
  if (!stack) return error.message
  if (error.message.length > 0 && !stack.includes(error.message)) {
    return `${error.message}\n${stack}`
  }
  return stack
}

/// 按**码点**截断（Array.from 按码点迭代）：按 UTF-16 码元 slice 会切出孤立
/// 代理项（emoji 等），JSON 序列化产生非法转义、serde_json 拒收，整条日志被丢。
function truncateByChars(text: string, max: number): string {
  const chars = Array.from(text)
  return chars.length <= max ? text : `${chars.slice(0, max).join('')}…`
}

/// data: URI → `data:<mime>`（载荷截断，MIME 缺失落 `data:uri`）。base64
/// 载荷可含攻击者可控内容，整段进日志既是泄漏面也是刷日志面。
function dataUriLabel(raw: string): string {
  const mime = /^data:([^;,]*)/.exec(raw)?.[1]
  return mime ? `data:${mime}` : 'data:uri'
}

/// CSP 违规来源标签：http(s) 只留 **host**（大批被拦资源按宿主合并去重，防
/// 挤占 frontend_log 的 60s 预算——文件名粒度由双报的 window error 行承担）；
/// data: 走 dataUriLabel（载荷不进日志）；blob: 溯源复杂只落 `blob:uri`；
/// inline/eval 白名单字面量原样。**不可解析串绝不原样返回**：相对地址可能带
/// 查询串令牌（`//?token=…` 实证曾整段进日志，与 fileNameOf 的教训同类），
/// 只保留 scheme 级信息，否则 unknown。
function cspSourceLabel(raw: unknown): string {
  if (typeof raw !== 'string' || raw.length === 0) return 'unknown'
  if (raw.startsWith('data:')) return dataUriLabel(raw)
  if (raw.startsWith('blob:')) return 'blob:uri'
  if (raw === 'inline' || raw === 'eval') return raw
  try {
    const url = new URL(raw)
    if (url.protocol === 'http:' || url.protocol === 'https:') return url.host
  } catch {
    const scheme = /^([a-z][a-z0-9+.-]*):/i.exec(raw)?.[1]
    return scheme ? `${scheme.toLowerCase()}:` : 'unknown'
  }
  return 'unknown'
}
