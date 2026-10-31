# frontend_log（前端日志落 Rust tracing）

> 源文件：`src-tauri/src/infra/frontend_log.rs`

## 职责

前端世界通往持久化日志的全部收敛与落盘逻辑：`commands/log_cmd.rs` 的 `frontend_log` 是薄转发，本模块完成「白名单收敛 → 会话级字节预算 → tracing 落盘」。前端守卫（`guards/globalErrorGuard`）把 Vue 渲染、window 未捕获异常与资源加载、未处理的 Promise 拒绝、路由错误四类错误送进来 —— panic 钩子的前端半边。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `emit` | `pub(crate) fn emit(level: &str, scope: &str, message: &str)` | 唯一入口：收敛 → 预算扣减 → 按级别以 `target: "frontend"` + `scope` 字段落 tracing |
| `sanitize_level` | `fn sanitize_level(level: &str) -> &'static str`（私有） | 级别白名单：`error`/`warn`/`info`/`debug`，其余一律按 `warn` 落（可见但不制造 error 噪音） |
| `sanitize_scope` | `fn sanitize_scope(scope: &str) -> &str`（私有） | scope 白名单：`[A-Za-z0-9._-]` 且 ≤ 64 字符，否则 `"<invalid>"` —— 同 `plugin_diagnostics::safe_id` 的模型 |
| `sanitize_message` | `fn sanitize_message(message: &str) -> String`（私有） | 消息收敛：保留换行/制表（前端上报的 `Error.stack` 由此可读），剥其它控制字符（防 `\r` 伪造行首、终端转义注入），经 `text::truncate_ellipsis` 按**字符**截断到 2000 |
| `consume_budget` | `fn consume_budget(bytes: u64) -> bool`（私有） | 会话级字节预算（`MAX_SESSION_BYTES = 64 KiB`，进程一次性、不随时间恢复）：CAS 扣减，耗尽后丢弃并**只在第一次丢弃时**告警一行（状态迁移值得看见，后续静默计数）。动机：前端去重只认完全相同的消息，渲染循环里带变化计数器的消息每条都是新串，无预算时一次循环即可刷满日志（5 MiB 上限只在启动时生效） |

## 直接依赖

- `super::text::truncate_ellipsis`（共享的按字符截断 + `…`，见 `infra/text.md`）
- `tracing`（`target: "frontend"` 落现有 logger：stderr + `logs/hinina.log`，受 `RUST_LOG` 过滤 —— 尽力而为通道，日志关闭时前端错误不留痕）
- `std::sync::atomic`（预算两个静态量；CAS 循环扣减）

## 被依赖

- `commands/log_cmd.rs`（`frontend_log` 薄转发；`main.rs` 注册）
- 前端 `src/bridge/diagnostics.bridge.ts` 的 `forwardFrontendLog`（唯一调用方，经 `guards/globalErrorGuard` 装配）

## 逻辑流程

前端守卫归一化错误（含截断后的 stack）→ `forwardFrontendLog` → IPC → 命令薄转发 → `emit`：

```
sanitize_level（白名单，未知→warn）
sanitize_scope（[A-Za-z0-9._-] ≤64，否则 "<invalid>"）
sanitize_message（去控制字符留 \n/\t，按字符截断 2000 + …）
consume_budget（64 KiB 会话预算，耗尽→丢弃 + 首次丢弃告警一行）
tracing::{error,warn,info,debug}!(target: "frontend", scope, "{message}")
```

**安全口径沿 `plugin_diagnostics`**：只收三个标量、无载荷参数（密码等敏感面从一开始就不存在）、scope 白名单化、消息限长防伪造与膨胀。

## 测试

`src/infra/tests/frontend_log_tests.rs`：级别白名单真值表（未知/大小写/空 → warn）；scope 白名单（合法、非法字符、超长、恰好 64、空）；消息按字符截断（2000/2001 边界 + `…`，中文不炸字节边界）；控制字符剥离（`\r`/`\x01` 替换为空格，`\n`/`\t` 保留）；**会话预算**（预算归零 → 丢弃且不 panic；小预算恰好耗尽 → 后续丢弃；大消息放行直到触底）—— 预算静态量是进程级单例，全部预算断言收进一个用例串行执行并以 `set_budget_for_tests` 重置，不与并行测试争状态。
