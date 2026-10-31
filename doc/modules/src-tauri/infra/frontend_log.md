# frontend_log（前端日志落 Rust tracing）

> 源文件：`src-tauri/src/infra/frontend_log.rs`

## 职责

前端世界通往持久化日志的全部收敛与落盘逻辑：`commands/log_cmd.rs` 的 `frontend_log` 是薄转发，本模块完成「白名单收敛 → 滚动窗口字节预算 → tracing 落盘」。前端守卫（`guards/globalErrorGuard`）把 Vue 渲染、window 未捕获异常与资源加载、未处理的 Promise 拒绝、路由错误四类错误送进来 —— panic 钩子的前端半边。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `emit` | `pub(crate) fn emit(level: &str, scope: &str, message: &str)` | 唯一入口：收敛 → 预算扣减 → 按级别以 `target: "frontend"` + `scope` 字段落 tracing |
| `sanitize_level` | `fn sanitize_level(level: &str) -> &'static str`（私有） | 级别白名单：`error`/`warn`/`info`/`debug`，其余一律按 `warn` 落（可见但不制造 error 噪音） |
| `sanitize_scope` | `fn sanitize_scope(scope: &str) -> &str`（私有） | scope 白名单：`[A-Za-z0-9._-]` 且 ≤ 64 字符，否则 `"<invalid>"` —— 同 `plugin_diagnostics::safe_id` 的模型 |
| `sanitize_message` | `fn sanitize_message(message: &str) -> String`（私有） | 消息收敛：保留换行/制表（前端上报的 `Error.stack` 由此可读），剥其它控制字符（防 `\r` 伪造行首、终端转义注入），经 `text::truncate_ellipsis` 按**字符**截断到 2000 |
| `consume_budget` | `fn consume_budget(bytes: u64) -> BudgetDecision`（私有） | **滚动窗口字节预算**（`BUDGET_WINDOW_BYTES = 64 KiB` / `BUDGET_WINDOW_SECS = 60`，窗口滚动即重置）：CAS 扣减，窗口内超限丢弃并返回 `Drop { first }`（本窗口首次丢弃由 `emit` 告警一行——状态迁移值得看见，后续静默计数；**告警后 return，首条超预算消息本身不落盘**）。窗口制而非进程一次性：失控循环被压到每窗口 64 KiB（持续刷屏最坏约 3.8 MB/小时，对照 5 MiB 启动轮转），**后续窗口里的真实错误恢复预算**，不因早期失控殃及整个会话。窗口滚动由 `roll_window_if_expired`（起点过期 ≥60s 时 CAS 推进并清零记账；CAS 失败=他人已滚动，不重复清零）；竞态窗口内记账是近似值——预算是闸门而非精确计量 |

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
consume_budget（64 KiB/60s 滚动窗口预算，超限→丢弃 + 本窗口首次丢弃告警一行）
tracing::{error,warn,info,debug}!(target: "frontend", scope, "{message}")
```

**安全口径沿 `plugin_diagnostics`**：只收三个标量、无载荷参数（密码等敏感面从一开始就不存在）、scope 白名单化、消息限长防伪造与膨胀。

## 测试

`src/infra/tests/frontend_log_tests.rs`：级别白名单真值表（未知/大小写/空 → warn）；scope 白名单（合法、非法字符、超长、恰好 64、空）；消息按字符截断（2000/2001 边界 + `…`，中文不炸字节边界）；控制字符剥离（`\r`/`\x01` 替换为空格，`\n`/`\t` 保留）；**滚动窗口预算**（`consume_budget` 直接断言 `BudgetDecision` 与内部记账快照 `budget_snapshot_for_tests`：新窗口记账、满窗口丢弃且首次丢弃 `first=true`/后续 `false`、过期窗口滚动清账恢复放行、59/60 秒过期边界）—— 预算静态量是进程级单例，全部预算断言收进**一个用例**（`budget_window_lifecycle`）串行执行并以 `set_budget_for_tests(seconds_ago, used, dropped)` 重置——预算静态量是进程级单例，而 Rust 测试在同一二进制内并行执行，两个用例各自 set→consume→snapshot 会互相踩状态（实测约 4.7% 失败率的竞态，评审抓出）。
