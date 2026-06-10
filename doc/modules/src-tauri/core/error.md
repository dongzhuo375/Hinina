# error

## 职责
定义应用全局错误类型 `AppError` 和 `AppResult<T>` 别名。`AppError` 覆盖 Auth、Contest、Problem、Submission、Workspace、Io、Network、Config、ProviderNotFound 等领域错误，可跨 Tauri IPC 序列化传递。

## 核心类型/函数
- **`AppError`** — 统一应用错误枚举，按领域分类共 10 个变体
- **`AppResult<T>`** — `Result<T, AppError>` 类型别名
- **`AppError::user_message()`** — 返回用户可读的错误信息
- **`AppError::is_user_facing()`** — 判断是否应向用户展示（当前恒为 `true`）

## 直接依赖
- `serde::Serialize`
- `thiserror::Error`

## 被依赖
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
所有 Service 与 Provider 通过 `AppResult<T>` 传播错误；前端通过 Tauri IPC 接收序列化后的 `AppError` 并展示 `user_message()`。
