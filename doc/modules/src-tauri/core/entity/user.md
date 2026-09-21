# user

## 职责
定义用户身份与会话实体 `User`，包含用户 ID、用户名和认证 token，支持 Serde 序列化以便跨 IPC 传输。

## 核心类型/函数
- **`User`** — 用户 struct，字段：`id`, `username`, `token`

## 直接依赖
- `serde::{Deserialize, Serialize}`

## 被依赖
- `core::provider::auth`（AuthProvider trait 使用 User 作为返回值）
- `service::auth`（`login` 的返回类型；注意 `CoreEvent::LoggedIn` **只带 `oj_id` 与 `user_id` 字符串**，不携带整个 `User` 实体）
- `commands::auth_cmd`

## 逻辑流程
无（纯类型定义）。
