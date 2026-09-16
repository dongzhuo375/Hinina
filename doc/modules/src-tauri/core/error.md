# error

## 职责
定义应用全局错误类型 `AppError` 和 `AppResult<T>` 别名。`AppError` 覆盖 Auth、Contest、Problem、Submission、Workspace、Io、Network、Config、ProviderNotFound、Serialization、Unknown 等领域错误，可跨 Tauri IPC 序列化传递。

## 核心类型/函数
- **`AppError`** — 统一应用错误枚举，按领域分类共 11 个变体。serde 采用**外部标签**表示，序列化为 `{"Auth": "消息"}` 这样的单键对象（不是 `{"variant":…, "message":…}`），前端 `bridge/index.ts` 的 `parseAppError` 依赖此形状归一化为 `IpcError`
- **`AppResult<T>`** — `Result<T, AppError>` 类型别名
- **`AppError::user_message()`** — 返回用户可读的错误信息（不含变体前缀）
- **`AppError::is_user_facing()`** — 判断是否应向用户展示（当前恒为 `true`）
- **`AppError::context(ctx)`** — 补上「哪个环节失败」的上下文，**逐变体保留原始变体**，只在原始消息前拼接环节名（`ctx: msg`）。为什么不能用 `AppError::Network(format!("xx 请求失败: {}", e))` 重新包装：那样会把反序列化失败、认证失败一律改写成「网络错误」，现场看到「网络错误: … 序列化错误: …」这类自相矛盾的嵌套消息，把排障引向错误方向；更严重的是**变体是前端 `isAuthError` 分流与 `sessionGuard` 会话失效兜底的唯一依据**，改写变体会让 401 不再触发登出，选手被卡在比赛页反复失败。取的是原始消息而非 `Display`，否则会重复带上「网络错误:」等前缀。**Service 层传播 Provider 错误一律用 `context()`，这是全项目约定，不得用 `AppError::Xxx(format!(...))` 重新包装** —— `service::{contest, problem, submission, auth}` 全部遵守（`get_rank` 是全场最高频的认证调用，每 10s 一次；`submit` / `login` 同理），`SubmissionService::get_judgement` 为单次查询、`Auth` 同样经 `context()` 直接上抛（后端不循环不重试，轮询节拍与瞬时错误容忍由前端 createPoller 拥有），会话失效不会被拖成「评测超时」
- **`prepend(ctx, msg)`**（私有）— `context()` 的消息拼接实现
- **`From` 转换** — `std::io::Error` → `Io`、`reqwest::Error` → `Network`、`serde_json::Error` → `Serialization`，让 `?` 自动转换

## 直接依赖
- `serde::Serialize`
- `thiserror::Error`

## 被依赖
- `adapter::hoj`（`context()` 补环节名、`Auth`/`Serialization` 变体判定）
- `commands::auth_cmd`
- `commands::config_cmd`
- `commands::contest_cmd`
- `commands::problem_cmd`
- `commands::submission_cmd`
- `commands::theme_cmd`
- `commands::workspace_cmd`
- `core::provider::auth`
- `core::provider::contest`
- `core::provider::problem`
- `core::provider::submission`
- `core::provider::registry`
- `core::repository::workspace_repo`
- `core::repository::config_repo`
- `core::repository::plugin_repo`
- `infra::fs_config_repo`
- `infra::fs_plugin_repo`
- `infra::fs_workspace_repo`
- `infra::http`（`status_error` 按 HTTP 状态码构造 `Auth`（401）/ `Network`（其余）变体；传输与读体失败构造 `Network`）
- `infra::provider_registry_impl`
- `service::auth`（`login` 用 `context()` 传播 Provider 错误）
- `service::contest`（`list_contests` / `get_rank` / `load_contest_with_problems` 用 `context()` 传播）
- `service::problem`（`list_problems` / `open_problem` / `get_user_problem_status` 用 `context()` 传播；`fetch_limits` 构造 `Unknown`）
- `service::submission`（`submit` / `get_judgement` / 三个查询方法均用 `context()` 传播，本层不构造变体）
- `service::workspace::manager`

## 逻辑流程
所有 Service 与 Provider 通过 `AppResult<T>` 传播错误；Adapter 在传输/解析失败时用 `context()` 补环节名（变体不变）；前端通过 Tauri IPC 接收序列化后的 `AppError`（`{ Variant: msg }` 形状），由 `bridge/index.ts` 归一化为 `IpcError` 后展示 `message`，并依据变体做认证失败分流。

**变体的产生与传播链**（每一层都只补消息、不改写变体）：

```
infra::http::status_error   HTTP 401 → Auth；其余状态码/传输失败 → Network
adapter::hoj                体内 401/403+登录提示 → Auth（auth_failure_from_body）
                            解析失败 → Serialization；DTO/业务失败 → Contest/Problem/Submission
                            传播时 e.context("HOJ xxx")
service::{contest,problem,submission,auth}
                            传播时 e.context("环节名")；仅本层自身语义才构造变体
                            （如 problem::fetch_limits → Unknown）
commands::*                 原样返回 AppResult，经 serde 序列化为 { Variant: msg }
前端 bridge/index.ts        parseAppError → IpcError；isAuthError(variant === 'Auth')
                            → stores/sessionGuard.ts 会话失效兜底 → 登出
```

## 测试
`src-tauri/src/core/tests/error_tests.rs`（在 `error.rs` 末尾以 `#[cfg(test)] #[path = "tests/error_tests.rs"] mod tests;` 引用）锁定：`context()` 对**全部 11 个变体**都保留变体且只在消息前拼接环节名（用穷尽 match 的 `variant_name` 兜底，新增变体时会漏测因此需同步补表）、`Auth` 错误经 `context()` 后仍可被 `matches!(_, AppError::Auth(_))` 识别（前端 sessionGuard 的判定依据）、`Display` 仍带变体前缀而 `user_message()` 不重复带前缀。
