# log_cmd（前端日志落 Rust tracing）

> 源文件：`src-tauri/src/commands/log_cmd.rs`

## 职责

承接前端全局错误转发的唯一 IPC 入口：`frontend_log` 命令把前端世界的错误（Vue 渲染、window 未捕获异常、未处理的 Promise 拒绝、路由错误）写进与 Rust panic 同一份 `logs/hinina.log` —— panic 钩子的前端半边。命令本身无状态、不依赖 `AppContext`。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `frontend_log` | `#[tauri::command] pub async fn frontend_log(level: String, scope: String, message: String) -> AppResult<()>` | 前端 invoke `frontend_log`({ level, scope, message })。按级别（error/warn/info/debug）以 `target: "frontend"` + `scope` 字段落 tracing；恒返回 `Ok`（入参已收敛，失败面不存在） |
| `sanitize_level` | `fn sanitize_level(level: &str) -> &'static str`（私有） | 级别白名单：`error`/`warn`/`info`/`debug`，其余一律按 `warn` 落（可见但不制造 error 噪音） |
| `sanitize_scope` | `fn sanitize_scope(scope: &str) -> &str`（私有） | scope 白名单：`[A-Za-z0-9._-]` 且 ≤ 64 字符，否则 `"<invalid>"` —— 同 `plugin_diagnostics::safe_id` 的模型，日志里只出现可归因的受控标识 |
| `sanitize_message` | `fn sanitize_message(message: &str) -> String`（私有） | 消息收敛：剥控制字符（保留 `\n`/`\t`，栈帧可读；防 `\r` 伪造行首与终端转义注入），超 2000 字符按**字符**截断加 `…`（中文不炸边界） |

## 直接依赖

- `tracing`（`target: "frontend"` 落现有 logger：stderr + `logs/hinina.log`，受 `RUST_LOG` 过滤 —— 尽力而为通道，日志关闭时前端错误不留痕）
- `crate::core::error::AppResult`

## 被依赖

- `main.rs`（`generate_handler!` 注册 `frontend_log`）
- 前端 `src/bridge/diagnostics.bridge.ts` 的 `forwardFrontendLog`（唯一调用方，经 `guards/globalErrorGuard` 装配）

## 逻辑流程

前端守卫归一化错误 → `forwardFrontendLog(level, scope, message)` → `ipcInvoke('frontend_log')` → 本命令三重收敛（level 白名单 / scope 白名单 / message 截断+去控制字符）→ tracing 按级别落日志。**不新增载荷参数**：前端不传 payload/args，敏感面（密码、页面内容）从一开始就不存在 —— 安全口径沿 `infra/plugin_diagnostics`（"永不格式化源错误文本"的前端版：只收标量、白名单、限长）。

## 测试

`src/commands/tests/log_cmd_tests.rs`：级别白名单真值表（未知/大小写不符/空 → warn）；scope 白名单（合法、非法字符、超长、恰好 64、空）；消息按字符截断（2000/2001 边界 + `…`，中文不炸字节边界）；控制字符剥离（`\r`/`\x01` 替换为空格，`\n`/`\t` 保留）。
