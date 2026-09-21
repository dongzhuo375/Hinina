# session

## 职责
本地会话记录实体 `Session`，持久化在 `sessions/{oj_id}.json`。**独立成实体的理由**：会话的写入方不止 `AuthService` —— Provider 侧凭证轮换（如 HOJ 的 Refresh-Token 协议）也必须在拿到新 token 的当场显式落盘。把 `Session` 与它的持久化契约（路径、字段名、兼容别名）收进领域层，才能让应用层与适配器层共用同一份 schema；否则适配器只能靠发布带真实 token 的事件、把落盘职责推给应用层的订阅者（旧实现的缺陷）。

## 核心类型/函数
- **`Session`** — `Debug + Clone + Serialize + Deserialize + PartialEq + Eq` 的会话记录：
  - `user_id: String` — 用户 ID（UUID）；带 `#[serde(default)]` 兼容升级前不含该字段的旧版会话文件（缺失时反序列化为空串）
  - `username: String`
  - `token: String` — 访问凭证。**绝不允许进入事件流**（见 `core/event/core_event.md` 的载荷约束）
  - `oj_id: String` — 会话归属的 OJ id；带 `#[serde(alias = "oj_type")]`，旧版文件键名为 `oj_type` 时兼容读取
- **`Session::new(oj_id, user_id, username, token) -> Self`** — 组装一条会话记录（`#[must_use]`）。参数顺序与字段声明顺序不同（oj_id 在前），调用方按语义传参即可。

## 直接依赖
- `serde::{Deserialize, Serialize}`

## 被依赖
- `core::repository::session_repo`（trait 的读写对象）
- `infra::fs_session_repo`（文件系统实现）
- `service::auth`（`pub use` re-export，命令层与自身流程共用）
- `adapter::hoj`（经 `SessionRepository` 间接使用：凭证轮换落盘）

## 逻辑流程
无（纯类型定义 + 一个字段搬运的构造函数）。真正的读写在 `infra::fs_session_repo`。

## 设计约束
- **schema 与旧版完全一致**（`{user_id, username, token, oj_id}`，`oj_id` 兼容 `oj_type` 旧键名）：v0.x 明文存储，v1.0 后再考虑加密 / 系统凭据管理器，因此既有会话文件**无需迁移**。
- **`token` 是本实体的敏感字段**：任何把它带进事件、日志或插件协议的做法都违反载荷约束；凭证轮换只发布「已轮换」这一事实（`CoreEvent::TokenRotated { oj_id }`）。

## 测试
`tests/session_tests.rs`：旧版文件（无 `user_id`）反序列化成功且该字段为空串、`oj_type` 旧键名经 alias 正确读到 `oj_id`、序列化输出 snake_case 键名（磁盘契约）、`Session::new` 按语义正确映射四个字段。
