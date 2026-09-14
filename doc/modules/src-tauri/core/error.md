# error

## 职责
定义应用全局错误类型 `AppError` 和 `AppResult<T>` 别名。`AppError` 覆盖 Auth、Contest、Problem、Submission、Workspace、Io、Network、Config、ProviderNotFound、Serialization、Unknown 等领域错误，可跨 Tauri IPC 序列化传递。

## 核心类型/函数
- **`AppError`** — 统一应用错误枚举，按领域分类共 11 个变体。serde 采用**外部标签**表示，序列化为 `{"Auth": "消息"}` 这样的单键对象（不是 `{"variant":…, "message":…}`），前端 `bridge/index.ts` 的 `parseAppError` 依赖此形状归一化为 `IpcError`
- **`AppResult<T>`** — `Result<T, AppError>` 类型别名
- **`AppError::user_message()`** — 返回用户可读的错误信息（不含变体前缀）
- **`AppError::is_user_facing()`** — 判断是否应向用户展示（当前恒为 `true`）
- **`AppError::context(ctx)`** — 补上「哪个环节失败」的上下文，**逐变体保留原始变体**，只在原始消息前拼接环节名（`ctx: msg`）。为什么不能用 `AppError::Network(format!("xx 请求失败: {}", e))` 重新包装：那样会把反序列化失败、认证失败一律改写成「网络错误」，现场看到「网络错误: … 序列化错误: …」这类自相矛盾的嵌套消息，把排障引向错误方向；更严重的是**变体是前端 `isAuthError` 分流与 `sessionGuard` 会话失效兜底的唯一依据**，改写变体会让 401 不再触发登出，选手被卡在比赛页反复失败。取的是原始消息而非 `Display`，否则会重复带上「网络错误:」等前缀
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
- `infra::provider_registry_impl`
- `service::workspace::manager`

## 逻辑流程
所有 Service 与 Provider 通过 `AppResult<T>` 传播错误；Adapter 在传输/解析失败时用 `context()` 补环节名（变体不变）；前端通过 Tauri IPC 接收序列化后的 `AppError`（`{ Variant: msg }` 形状），由 `bridge/index.ts` 归一化为 `IpcError` 后展示 `message`，并依据变体做认证失败分流。

## 测试
`src-tauri/src/core/tests/error_tests.rs`（在 `error.rs` 末尾以 `#[cfg(test)] #[path = "tests/error_tests.rs"] mod tests;` 引用）锁定：`context()` 对**全部 11 个变体**都保留变体且只在消息前拼接环节名（用穷尽 match 的 `variant_name` 兜底，新增变体时会漏测因此需同步补表）、`Auth` 错误经 `context()` 后仍可被 `matches!(_, AppError::Auth(_))` 识别（前端 sessionGuard 的判定依据）、`Display` 仍带变体前缀而 `user_message()` 不重复带前缀。
