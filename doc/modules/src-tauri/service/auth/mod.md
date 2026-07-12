# mod

## 职责
认证服务模块入口。负责登录流程编排、会话持久化、登出清理及会话有效性验证。会话以 JSON 格式存储在 `sessions/{oj_type}.json`，v0.x 明文存储。通过 `ProviderRegistry` 获取当前 OJ 的 AuthProvider，支持运行时 OJ 切换后自动适配。

## 核心类型/函数
- `pub mod error` — 认证错误类型模块声明
- **`Session`** — 本地会话记录结构体（`username`, `token`, `oj_type`），实现 `Serialize + Deserialize`
- **`AuthService`** — 认证服务
  - `fn new(registry, storage, event_bus) -> Self` — 创建实例
  - `async fn login(&self, username, password) -> AppResult<User>` — 调用 AuthProvider 登录 → 持久化 session → 发布 `AuthEvent::LoginSuccess`
  - `async fn logout(&self) -> AppResult<()>` — 远端登出（非致命）→ 删除本地会话文件 → 发布 `AuthEvent::Logout`
  - `fn get_session(&self) -> Option<Session>` — 从本地文件恢复会话；文件不存在或 JSON 解析失败返回 `None`
  - `async fn validate_session(&self) -> bool` — 本地恢复 session → 远端校验；会话过期时发布 `AuthEvent::SessionExpired`
- **字段**：`registry: Arc<dyn ProviderRegistry>`, `storage: Arc<Storage>`, `event_bus: Arc<EventBus>`
- 常量：`SESSIONS_DIR: &str = "sessions"`
- 内部方法：`save_session()`, `session_path_str()`, `session_path()`

## 直接依赖
- `core::entity::user::User`
- `core::error::{AppError, AppResult}`
- `core::event::app_event::{AppEvent, AuthEvent}`
- `core::event::event_bus::EventBus`
- `core::provider::registry::ProviderRegistry`
- `infra::storage::Storage`

## 被依赖
- `core::context`（`AppContext` 持有 `Arc<AuthService>`）

## 逻辑流程
- **login**：调用 `ProviderRegistry::get_auth()` → `AuthProvider::login()` → 本地持久化 session JSON → 发布 `LoginSuccess` 事件
- **logout**：尝试远端 `logout()`（失败仅警告）→ 删除 `sessions/{oj_type}.json` → 发布 `Logout` 事件
- **get_session**：读取 `sessions/{oj_type}.json` 反序列化为 `Session`；文件不存在或损坏返回 `None`
- **validate_session**：先 `get_session()` → 无本地会话返回 `false`；调用远端 `validate_session()` → 过期时发布 `SessionExpired`
