# globalErrorGuard（全局错误转发守卫）

> 源文件：`src/guards/globalErrorGuard.ts`

## 职责

把四类「JS 世界的崩溃」统一转发到 Rust 日志：Vue 组件树错误（`app.config.errorHandler`）、未捕获同步异常与资源加载失败（`window` `error` 事件）、无人接的 Promise 拒绝（`unhandledrejection`）、路由错误（`router.onError`）。此前它们只进 console —— 用户不开 DevTools 就**白屏无痕**，Rust 日志完全看不到 JS 世界；这是 panic 钩子的前端半边（Rust 侧入口为 `commands/log_cmd.rs` 的 `frontend_log`）。

## 落位说明（为什么在 `guards/`）

与 `sessionGuard` 同款跨层装配：需要 `app`（Vue 实例）与 `router`，且是应用级横切关注点而非领域服务。进 `services/` 会造成 Service → Store 反向依赖，进 `stores/` 名不符实（无 `defineStore`）。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `installGlobalErrorGuard` | `(app: App, router: Router, forward: FrontendLogForward) => () => void` | 装四个钩子，返回清理函数（移除监听、`errorHandler` 复位、清去重表）—— 仅供测试拆卸；组合根不拆卸。**必须先于 mount 调用**：errorHandler 不在位时首屏渲染错误直接逃逸。window `error` 监听**必须注册在 capture 阶段**（注册与移除均传 `capture: true`）：资源加载失败的 error 事件派发在元素上且**不冒泡**，bubble 阶段的 window 监听永远收不到（未捕获异常的 error 事件派发在 window 自身，target 阶段，不受影响）。errorHandler **补发 `console.error(err, info)`** —— Vue 设了 handler 后不再走默认输出，不补这一行 DevTools 里组件错误就消失（转发是持久化，console 是现场排障） |
| `FrontendLogForward` | `(level: FrontendLogLevel, scope: string, message: string) => void` | 注入的转发出口；组合根接到 `bridge/diagnostics.bridge` 的 `forwardFrontendLog` |
| `report` | `(scope, error, detail?) => void`（模块内私有） | 核心上报：归一化 → 限频去重 → 转发，全段 try/catch —— **守卫自身绝不抛错**（报错的通道不允许自己成为新的错误源） |
| `normalize` | `(error: unknown, detail?) => string`（模块内私有） | 归一化：`Error` → **message + stack**（V8 首行自带消息即直接用 stack；SpiderMonkey 不带消息头则补一行；栈由 2000 字符上限自然截取头部若干帧 —— Rust 端为保留 `
`/`	` 做的处理由此真正生效）；IpcError 鸭子识别（`cmd`+`message`）→ `ipc:{cmd}: message`；空/null → `<empty error>`；其余 String 化；统一按**码点**截断 2000 字符（`Array.from`，UTF-16 码元 slice 会切出孤立代理项 → serde_json 拒收、整条日志丢弃；Rust 侧再兜底） |
| 限频去重 | `lastSeen: Map<key, ts>`（模块内私有） | 同签名（scope + **完整归一化消息**——截前缀会把「同前缀 + 变化后缀」的不同错误误并成一条）1 秒内只转发一次；表容量 256，超限整表清空。**forward 同步抛错会撤销去重记录** —— 这条没送出去，同签名错误在窗口内可重试。**内容有变化的消息（如带变化计数器）不在此去重**，由 Rust 端 `frontend_log` 的滚动窗口字节预算兜底（64 KiB/60s，见 `infra/frontend_log.md`） |

## 直接依赖

- `vue`（仅 `App` 类型）
- `vue-router`（`Router` 类型 + `onError`）
- `@/bridge/diagnostics.bridge`（仅 `FrontendLogLevel` 类型；转发函数由组合根注入，本模块不 import 其实现）
- `@/bridge`（仅 `IpcError` 类型，鸭子识别用）

## 被依赖

- `main.ts` — 唯一生产消费方（第一个装配，先于 `installSessionGuard`）
- `src/guards/__tests__/globalErrorGuard.spec.ts` / `src/__tests__/main.spec.ts`

## 逻辑流程

```
四类钩子触发
  errorHandler(err, _instance, info)        → report('vue',     err, `vue:${info}`)
  window 'error'（capture 阶段；有 error/message）→ report('window', source)
  window 'error'（capture 阶段；无消息 → 资源加载失败）→ report('window', `资源加载失败: <tag> 文件名`)
  unhandledrejection(reason)                → report('promise', reason)
  router.onError(error)                     → report('router', error)
→ normalize（归一 + 截断）→ 限频去重 → forward('error', scope, message)
→ 组合根的 forward 包装 → forwardFrontendLog → Rust frontend_log → tracing → logs/hinina.log
```

设计要点：

- **守卫零信任自身**：`report` 整体 try/catch，`forward` 同步抛错被单独捕获并撤销去重记录（重试窗口）；绝不 `preventDefault` unhandledrejection —— 默认 console 输出保留，本守卫是补一份持久化，不接管 DevTools 排障。
- **安全边界**：只转发 level/scope/message 三个标量，不传 payload/args；scope 来自本模块常量（`vue`/`window`/`promise`/`router`），长度与字符白名单由 Rust 端收敛（`infra/frontend_log.md`）。资源加载失败**只记标签名与路径末段文件名** —— 绝对 URL（Markdown 外链图片）可能带签名令牌等查询参数，完整 URL 不进日志。
- **持久化是尽力而为**：`RUST_LOG=off` 或日志文件不可写时前端错误不留痕（与 panic 的 bootstrap 文件不同，不承诺无条件落盘）；转发失败在组合根降级为 `log.warn`。

## 测试

`src/guards/__tests__/globalErrorGuard.spec.ts`（jsdom，12 用例）：四类钩子触发 → forward 收到归一化消息（路由错误用「导航守卫抛错」确定性触发；Error 断言用包含式 —— 消息含 stack）；**errorHandler 补发 console.error**（`toHaveBeenCalledWith(err, info)`）；Error 上报携带堆栈（含 `at ` 帧）；**按码点截断**（emoji 落在截断边界，断言完整代理对存活）；**去重键用完整消息**（同 300 字符前缀不同后缀的两条都转发）；拆卸后不再转发；IpcError 归一；**资源加载错误用真实 DOM 派发验证**（元素挂进文档、事件派发在元素上且 `bubbles: false` —— 无 capture 的实现下该用例必红，变异验证过；伪造 `event.target` 直接向 window 派发只会走 target 阶段，测不出传播路径），且**只记文件名不记 URL**（带 `?token=SECRET` 的外链断言 `SECRET`/`http` 不在消息里）；限频去重（fake timers + 1s 窗口 + 窗口后重放，用字符串 reason —— Error 的 stack 含构造行号，两次构造天然不同键）；forward 同步抛错被吞、窗口后同签名可重试；空/非 Error reason 归一。每个用例自建 app+router 并在 afterEach 统一拆卸 —— 单个断言失败不得把监听器泄漏进后续用例。装配面由 `src/__tests__/main.spec.ts` 锁定（先于 sessionGuard 装配）。
