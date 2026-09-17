# logger（前端作用域日志）

> 源文件：`src/utils/logger.ts`

## 职责

前端日志的唯一入口：统一作用域前缀、按级别分流（`debug`/`info` 仅开发环境，`warn`/`error` 始终输出）。**不做上报与落盘** —— 客户端持久化日志由 Rust 侧 `tracing` 负责，前端 console 仅供开发与现场 DevTools 排障。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `Logger` | `{ debug, info, warn, error: (...args: unknown[]) => void }` | 日志器接口；参数原样透传（便于 DevTools 展开对象），不序列化 |
| `createLogger` | `(scope: string) => Logger` | 创建带 `[scope]` 前缀的日志器；作用域名沿用模块名（`configService` / `workspaceStore` / `ipc` …），与既有日志文案一致，便于按模块过滤现场日志 |
| `IS_DEV` | `import.meta.env.DEV`（模块私有） | 级别分流判据（`vite build` 产物为 false，vitest 为 true） |

## 直接依赖

无（仅使用 `import.meta.env`，类型来自 `vite/client`）。

## 被依赖

- `bridge/index.ts`（`ipc` —— IPC 失败单点日志）
- `services/auth.service.ts`、`services/config.service.ts`
- `stores/authStore.ts` / `announcementStore.ts` / `problemStore.ts` / `submissionStore.ts` / `workspaceStore.ts` / `sessionGuard.ts`
- `views/ProblemSolveView.vue` / `SubmissionDetailView.vue`
- `components/editor/CodeEditor.vue` / `EditorConsoleBar.vue`
- `components/problem/ProblemStatement.vue` / `QuickSubmitDialog.vue`

## 逻辑流程

```
createLogger('workspaceStore') → log
log.debug(...)  → IS_DEV 时 console.debug('[workspaceStore]', ...)，否则 no-op
log.info(...)   → 同上（console.info）
log.warn(...)   → 恒 console.warn('[workspaceStore]', ...)
log.error(...)  → 恒 console.error('[workspaceStore]', ...)
```

设计要点：

- **warn/error 不按环境关闭**：现场排障时用户不会开 DevTools，Rust 侧 tracing 只覆盖后端；
  前端能留下线索的只有这两级。
- **不做日志上报**：客户端无日志服务端（项目边界明确不依赖服务器），加队列/上报只会引入
  新的失败面。
- **安全约束与 `bridge/ipcInvoke` 一致**：日志只记录命令名、错误消息与业务标识，
  **绝不记录敏感参数**（登录密码等）。
- 作用域前缀由 logger 统一添加，调用点不再手写 `[模块名]`，避免同一模块出现两种前缀写法。

## 测试

`src/utils/__tests__/logger.spec.ts`：`[scope]` 前缀与原始参数透传、warn/error 恒输出、debug/info 仅开发环境、多作用域互不串扰。
