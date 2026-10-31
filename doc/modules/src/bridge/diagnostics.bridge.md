# diagnostics.bridge（前端日志转发桥）

> 源文件：`src/bridge/diagnostics.bridge.ts`

## 职责

前端世界通往 Rust 日志的唯一桥：`forwardFrontendLog` 把一条标量日志（level/scope/message）经 `ipcInvoke('frontend_log')` 交给 Rust `tracing`（stderr + `logs/hinina.log`）。消费方唯一：`guards/globalErrorGuard`（经组合根注入）。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `FrontendLogLevel` | `type FrontendLogLevel = 'debug' \| 'info' \| 'warn' \| 'error'` | 级别闭集（与 Rust 端 `frontend_log` 白名单一致；未知级别后端按 warn 落） |
| `forwardFrontendLog` | `(level, scope, message) => Promise<void>` | invoke `frontend_log`。**失败语义按约定不在此处吞**：返回的 Promise 可能拒绝（后端不可用等），由调用守卫吞掉 —— 报错的通道不允许自己抛错打断错误源 |

## 直接依赖

- `@/bridge`（`ipcInvoke` —— IPC 唯一出口；其单点日志与错误观察者对本命令照常生效，`frontend_log` 自身的失败不会误触会话守卫）

## 被依赖

- `main.ts`（组合根把 `forwardFrontendLog` 包上 catch 后注入 `installGlobalErrorGuard`）
- `src/bridge/__tests__/diagnostics.bridge.spec.ts`

## 逻辑流程

`forwardFrontendLog(level, scope, message)` → `ipcInvoke('frontend_log', { level, scope, message })` → Rust `commands/log_cmd.rs` 白名单收敛 → tracing（`target: "frontend"`）。安全约束与全链一致：只传三个标量，**不传 payload**（密码、页面内容从一开始就不进入此通道）；长度与字符白名单由 Rust 端收敛，前端不重复实现。

## 测试

`src/bridge/__tests__/diagnostics.bridge.spec.ts`：mock `@tauri-apps/api/core` 的 `invoke`，断言命令名与三标量参数透传；后端失败时按约定向上抛 `IpcError`（`cmd: 'frontend_log'`），由守卫吞掉。
