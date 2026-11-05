# app_cmd（应用级命令：启动标志 + 外链转交）

> 源文件：`src-tauri/src/commands/app_cmd.rs`

## 职责

把 `infra::launch_args` 解析的启动标志交给前端。前端守卫据此做 `--settings` 的初始导航直落与登录门禁豁免，设置页据此决定分层锁（哪些字段禁用）。

外链（`open_external`）：把题面/公告 markdown 里的 http(s) 链接转交**系统默认浏览器**——WebView 内点这类链接的默认行为是把整个客户端导航走（分层审计 2026-10「外链无出路」缺口）。前端拦截在 `src/utils/external-links`（document 级委托，main.ts 装配）；本命令是转交通道。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `launch_flags` | `#[tauri::command] pub async fn launch_flags() -> AppResult<LaunchFlags>` | 返回**本次启动**的标志快照 `{ settings: bool }`。**零状态命令**（沿 `log_cmd::frontend_log` 先例，不取 `State<AppContext>`）——标志在 main() 早期由 `launch_args::init` 固化，命令只读。IPC 失败时前端按「无参数」退化（正常门禁 + 选手层），绝不让钥匙通道故障卡死启动或设置页 |
| `open_external` | `#[tauri::command] pub async fn open_external(app: AppHandle, url: String) -> AppResult<()>` | 把 http(s) 外链转交系统默认浏览器：`validate_external_url` 校验 → `app.opener().open_url(...)`（`tauri-plugin-opener` 的 **Rust API**；插件 Rust-only 注册，不装 npm 包、不声明 opener capability——插件的 IPC 命令虽已在 invoke handler 注册，但 webview 调用一律被 ACL 拒绝；注册时并关闭默认开启的 `open_js_links_on_click`，插件不注入点击脚本，本命令是唯一转交通道）。**scheme 白名单由本命令守卫**（`Opener` Rust API 无 scope 校验，官方文档明示）；日志只记 host 不记完整 URL（query 可能带签名令牌）；失败 `AppError::Unknown`，前端 catch 后仅记日志（无补救动作，窗口控制同款） |
| `validate_external_url` | `pub(crate) fn validate_external_url(url: &str) -> Result<url::Url, String>` | scheme 白名单纯函数（只放行 http/https，`url` crate 解析）。**错误信息刻意不含原始 URL**（令牌安全）。独立成纯函数以便单测——真实 open 不做单测（tauri mock runtime 污染测试二进制，single_instance 先例），真机人工验收 |

## 直接依赖

- `crate::infra::launch_args`（`get` / `LaunchFlags`）
- `crate::core::error::{AppError, AppResult}`
- `tauri::AppHandle`、`tauri_plugin_opener::OpenerExt`（外链转交；crate 见 `Cargo.toml`）
- `url` crate（`validate_external_url` 解析；已是 reqwest 的直接依赖）
- `tracing::info`（host-only 日志）

## 被依赖

- `main.rs` 的 `generate_handler!`（命令清单首位，注册命令之一）
- 前端 `src/bridge/system.bridge.ts` 的 `getLaunchFlags()` / `openExternal()`（经 `system.service` 转发；`openExternal` 的消费方是 main.ts 组合根的外链拦截装配 `utils/external-links`）

## 逻辑流程

- `invoke('launch_flags')` → 本命令同步读 `launch_args::get()` → 序列化为 `{ "settings": bool }` 返回。无参数、无状态、无副作用；不写日志（标志本身不含敏感信息，但也无记录必要）。
- `invoke('open_external', { url })` → `validate_external_url`（http/https 白名单；失败 `AppError::Config`，错误信息不含 URL）→ `app.opener().open_url(...)` 转交系统浏览器 → `info!(host)`。前端拦截层（`utils/external-links`）已保证只有解析后 http/https 且 host ≠ 应用 host 的锚点会进来。

## 测试

`launch_flags`：无独立命令单测，解析语义与默认值行为由 `infra/tests/launch_args_tests.rs` 全覆盖；线上形状由 `LaunchFlags` Serialize 与前端 `types/system.ts` 两侧对齐。

`validate_external_url`：`tests/app_cmd_tests.rs`（4 用例）——接受 http/https；拒绝 file/javascript/data/ftp/chrome/UNC 等非白名单 scheme 与畸形 URL；**错误信息不含原始 URL**（令牌安全断言）。scheme 白名单变异验证过（移除白名单 → 用例转红）。真实 open 不做单测（mock runtime 先例），真机人工验收：点题面外链 → 系统浏览器打开且应用界面不动。
