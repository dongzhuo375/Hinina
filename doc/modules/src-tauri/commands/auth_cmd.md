# auth_cmd

## 职责
认证相关 Tauri Command 模块。处理用户登录、登出及会话查询，面向前端暴露 `auth:*` 命名空间的 IPC 接口。

## 核心类型/函数
- `pub async fn login(ctx, username, password, oj_type) -> AppResult<User>` — 登录指定 OJ 平台
- `pub async fn logout(ctx) -> AppResult<()>` — 登出当前会话
- `pub async fn get_session(ctx) -> AppResult<Option<User>>` — 查询当前登录会话

## 直接依赖
- `tauri::State`
- `crate::core::context::AppContext`
- `crate::core::entity::user::User`
- `crate::core::error::AppResult`

## 被依赖
- `src-tauri/src/commands/mod.rs`（通过 `pub mod auth_cmd` 声明）

## 逻辑流程
1. 前端通过 `invoke('auth:login', {...})` 调用对应 Tauri command
2. 各 command 接收 `AppContext` State 及业务参数
3. 当前所有函数均为占位实现（`todo!()`），待接入具体 AuthProvider 实现
