# mod

## 职责
认证服务模块入口。负责登录流程编排、会话持久化、登出清理及会话有效性验证。会话以 JSON 格式存储在 `sessions/{oj_type}.json`，v0.x 明文存储。通过 `ProviderRegistry` 获取当前 OJ 的 AuthProvider，支持运行时 OJ 切换后自动适配。

## 核心类型/函数
- `pub mod error` — 认证错误类型模块声明
- **`Session`** — 本地会话记录结构体（`user_id`, `username`, `token`, `oj_type`），实现 `Serialize + Deserialize`
- **`SessionValidity`** — 会话校验三态结果（`Valid` / `Invalid` / `Unknown`），`#[serde(rename_all = "snake_case")]` 序列化为 `valid` / `invalid` / `unknown`，属跨端契约。区分"服务端明确失效"与"无法判定"至关重要：后者若按失效处理，会在赛前把选手踢回登录页，反复重登还可能触发 HOJ 的暴力破解锁定（同 IP + 同用户名 30 分钟 20 次）
- **`AuthService`** — 认证服务
  - `fn new(registry, storage, event_bus) -> Self` — 创建实例，并订阅 `EventCategory::Auth` 以处理凭证轮换
  - `async fn login(&self, username, password) -> AppResult<User>` — 调用 AuthProvider 登录 → 持久化 session → 发布 `AuthEvent::LoginSuccess`。登录失败经 `e.context("登录失败")` 上抛，**保留原始变体**
  - `async fn logout(&self) -> AppResult<()>` — 远端登出（非致命）→ 删除本地会话文件 → 发布 `AuthEvent::Logout`
  - `fn get_session(&self) -> Option<Session>` — 从本地文件恢复会话；恢复成功时将 token 回注到 Provider（`AuthProvider::restore_token`），保证重启后认证请求仍携带 Authorization 头；文件不存在或 JSON 解析失败返回 `None`
  - `async fn validate_session(&self) -> SessionValidity` — 本地恢复 session（同时回注 token）→ 远端校验；无本地会话或服务端明确判定失效返回 `Invalid`（后者清除磁盘会话并发布 `AuthEvent::SessionExpired`），网络异常/无 Provider 返回 `Unknown`（保留本地会话）
- **字段**：`registry: Arc<dyn ProviderRegistry>`, `storage: Arc<Storage>`, `event_bus: Arc<EventBus>`
- 常量：`SESSIONS_DIR: &str = "sessions"`
- 内部方法：`subscribe_token_refresh()`（订阅 `TokenRefreshed` 事件，将新 token 回写当前 OJ 的磁盘会话；无会话文件时静默跳过）, `save_session()`, `clear_session()`（幂等删除会话文件）, `session_path_str()`, `session_path()`

## 直接依赖
- `core::entity::user::User`
- `core::error::AppResult`（**不再直接引用 `AppError`**：`login` 传播 Provider 错误只用 `e.context(...)`，不构造新变体）
- `core::event::app_event::{AppEvent, AuthEvent}`
- `core::event::event_bus::EventBus`
- `core::event::event_category::EventCategory`
- `core::provider::registry::ProviderRegistry`
- `infra::storage::Storage`

## 被依赖
- `core::context`（`AppContext` 持有 `Arc<AuthService>`）
- `commands::auth_cmd`（`login` / `logout` / `get_session` / `validate_session` 四个 Command 转发；并复用 `SessionValidity` 类型）

## 逻辑流程
- **login**：调用 `ProviderRegistry::get_auth()` → `AuthProvider::login()`（失败 `warn!` + `e.context("登录失败")`，**变体原样穿透**）→ 本地持久化 session JSON → 发布 `LoginSuccess` 事件
- **logout**：尝试远端 `logout()`（失败仅警告）→ 删除 `sessions/{oj_type}.json` → 发布 `Logout` 事件
- **get_session**：读取 `sessions/{oj_type}.json` 反序列化为 `Session`；恢复成功后调用 `ProviderRegistry::get_auth()` 获取 Provider 并回注 token；文件不存在或损坏返回 `None`
- **validate_session**：先 `get_session()`（无本地会话 → `Invalid`；有则顺带把 token 回注 Provider，否则重启后的校验请求必然 401）→ 取不到 AuthProvider → `Unknown`；调用远端 `validate_session()`：`Ok(true)` → `Valid`；`Ok(false)` → `clear_session()` 删除磁盘会话 + 发布 `SessionExpired` → `Invalid`；`Err(_)`（网络异常）→ `Unknown` 并保留本地会话
- **TokenRefreshed 订阅**：构造时注册 `EventCategory::Auth` 订阅；事件到达时读取当前 OJ 磁盘会话 → 更新 token 字段 → 回写。会话不存在（如轮换发生在登录持久化之前的极端时序）静默跳过。订阅句柄随 AuthService 进程级生命周期共存

## 设计要点
- **`login` 用 `context()` 保留变体**：密码错误（`Auth`）与服务器连不上（`Network`）对用户的处置完全不同 —— 前者要提示检查凭证，后者要提示检查网络/稍后重试。此前一律改写成 `AppError::Auth(format!("登录失败: {}", e))` 会让「服务器连不上」显示成「认证错误」，选手在赛前反复改密码重登，还可能撞上 HOJ 的暴力破解锁定（同 IP + 同用户名 30 分钟 20 次）。这也是全项目约定：Service 层传播 Provider 错误一律 `e.context("…")`，不得重新包装（见 `core/error.md`）。
- **三态契约由 Adapter 兑现，Service 只做映射**：`validate_session` 的 `Ok(true)`/`Ok(false)`/`Err(_)` → `Valid`/`Invalid`/`Unknown` 映射本身一直是正确的；`Err(_)` 分支能否被走到，取决于各 OJ Adapter 是否真的会在「无法判定」时返回 `Err`（HOJ 侧的判据见 `adapter/hoj/mod.md` 的 `session_validity_from_response`）。

## 测试
`src-tauri/src/service/auth/tests/auth_tests.rs` 锁定：会话保存/恢复 roundtrip（含 token 回注）、`TokenRefreshed` 事件回写磁盘会话（无会话静默跳过、多订阅者都能收到）、logout 删除磁盘会话、损坏会话文件返回 `None`、`clear_session` 幂等、`validate_session` 三态全覆盖（无本地会话/无 Provider → Invalid/Unknown、有效恢复 token、明确失效清会话并发事件、网络错误 Unknown 保留会话）、`SessionValidity` snake_case 序列化契约。

测试文件现**显式** `use crate::core::error::AppError;`：`mod.rs` 已不再直接引用 `AppError`，故 `use super::*` 不再把它带进作用域（`StubAuthProvider::validate_session` 的 `NetworkErr` 分支需要构造 `AppError::Network`）。

`StubAuthProvider` 以 `StubOutcome::{Valid, Invalid, NetworkErr}` 预设校验结果，对应的三态测试证明 **Service 侧映射一直是正确的** —— `validate_session` 三态失效的 bug 不在本模块，而在 HOJ adapter 此前把网络异常折成 `Ok(false)`、永不返回 `Err`，使 `Unknown` 分支对 HOJ 成为死代码（修复见 `adapter/hoj/mod.md`）。这组用例因此是「Service 契约」的回归护栏，与 adapter 侧的 `validity_*` 用例互补，两者都不可省。
