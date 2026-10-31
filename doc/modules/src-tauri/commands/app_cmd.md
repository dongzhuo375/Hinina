# app_cmd（应用级启动标志）

> 源文件：`src-tauri/src/commands/app_cmd.rs`

## 职责

把 `infra::launch_args` 解析的启动标志交给前端。前端守卫据此做 `--settings` 的初始导航直落与登录门禁豁免，设置页据此决定分层锁（哪些字段禁用）。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `launch_flags` | `#[tauri::command] pub async fn launch_flags() -> AppResult<LaunchFlags>` | 返回**本次启动**的标志快照 `{ settings: bool }`。**零状态命令**（沿 `log_cmd::frontend_log` 先例，不取 `State<AppContext>`）——标志在 main() 早期由 `launch_args::init` 固化，命令只读。IPC 失败时前端按「无参数」退化（正常门禁 + 选手层），绝不让钥匙通道故障卡死启动或设置页 |

## 直接依赖

- `crate::infra::launch_args`（`get` / `LaunchFlags`）
- `crate::core::error::AppResult`

## 被依赖

- `main.rs` 的 `generate_handler!`（命令清单首位，47 个 Command 之一）
- 前端 `src/bridge/system.bridge.ts` 的 `getLaunchFlags()`（唯一消费方，经 `system.service` 的 memoize 供守卫与设置页共用）

## 逻辑流程

前端 `invoke('launch_flags')` → 本命令同步读 `launch_args::get()` → 序列化为 `{ "settings": bool }` 返回。无参数、无状态、无副作用；不写日志（标志本身不含敏感信息，但也无记录必要）。

## 测试

无独立命令单测：命令体只有一行只读转发，解析语义与默认值行为由 `infra/tests/launch_args_tests.rs` 全覆盖（见 `infra/launch_args.md` 测试节）。线上形状 `{ settings }` 由 `LaunchFlags` 的 Serialize 派生与前端 `types/system.ts` 的 `LaunchFlags` 接口两侧对齐。
