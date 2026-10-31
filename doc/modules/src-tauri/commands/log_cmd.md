# log_cmd（前端日志落 Rust tracing · 薄转发）

> 源文件：`src-tauri/src/commands/log_cmd.rs`

## 职责

承接前端全局错误转发的唯一 IPC 入口：`frontend_log` 命令把前端世界的错误（Vue 渲染、window 未捕获异常与资源加载、未处理的 Promise 拒绝、路由错误）写进与 Rust panic 同一份 `logs/hinina.log` —— panic 钩子的前端半边。**薄转发**（CLAUDE.md：commands 保持轻量、委托底层）：收敛（级别/scope 白名单、消息截断与控制字符剥离）、会话级字节预算与 tracing 落盘全部在 `infra/frontend_log.rs`，本命令一行委托。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `frontend_log` | `#[tauri::command] pub async fn frontend_log(level: String, scope: String, message: String) -> AppResult<()>` | 前端 invoke `frontend_log`({ level, scope, message })。委托 `infra::frontend_log::emit`，恒返回 `Ok` |

## 直接依赖

- `crate::infra::frontend_log::emit`（全部逻辑所在，见 `infra/frontend_log.md`）

## 被依赖

- `main.rs`（`generate_handler!` 注册 `frontend_log`）
- 前端 `src/bridge/diagnostics.bridge.ts` 的 `forwardFrontendLog`（唯一调用方，经 `guards/globalErrorGuard` 装配）

## 逻辑流程

前端守卫归一化错误 → `forwardFrontendLog` → `ipcInvoke('frontend_log')` → 本命令 → `infra::frontend_log::emit`（白名单收敛 + 会话预算 + tracing，`target: "frontend"`）。安全口径（白名单、限长、无载荷参数）见 `infra/frontend_log.md`。

## 测试

命令本体是零逻辑薄转发，行为测试全部在 `infra/tests/frontend_log_tests.rs`（见 `infra/frontend_log.md`）。
