# user

## 职责
定义用户身份与会话实体 `User`，包含用户 ID、用户名和认证 token，支持 Serde 序列化以便跨 IPC 传输。

## 核心类型/函数
- **`User`** — 用户 struct，字段：`id`, `username`, `token`

## 直接依赖
- `serde::{Deserialize, Serialize}`

## 被依赖
- `core::event::app_event`（AuthEvent::LoginSuccess 携带 User）
- `core::provider::auth`（AuthProvider trait 使用 User 作为返回值）
- `commands::auth_cmd`

## 逻辑流程
无（纯类型定义）。
