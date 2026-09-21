# mod

## 职责
认证服务模块入口。负责登录流程编排、会话持久化、登出清理及会话有效性验证。会话以 JSON 格式存储在 `sessions/{oj_id}.json`（如 `sessions/HOJ.json`，内建 OJ 的 id 与历史枚举 Debug 输出一致，旧会话文件无需迁移），v0.x 明文存储；读写经 `SessionRepository` 仓库（**本服务不再直接持有 `Storage`**）。通过 `ProviderRegistry` 获取当前 OJ 的 AuthProvider（`current_auth()` / `current_id()`），支持运行时 OJ 切换后自动适配。

**本服务不订阅任何事件**：它承担的每一项工作（落盘、清理）都必须显式完成，不能交给异步消费者。`LoggedIn` / `LoggedOut` / `SessionExpired` 只是「这些动作已经做完」的事实通知。

## 核心类型/函数
- `pub mod error` — 认证错误类型模块声明
- **`Session`** — 本地会话记录，**由 `core::entity::session` re-export**（`pub use crate::core::entity::session::Session;`）：领域层与适配器层共用同一份 schema，本模块不再自带一份定义。字段与兼容别名见 `core/entity/session.md`
- **`SessionValidity`** — 会话校验三态结果（`Valid` / `Invalid` / `Unknown`），`#[serde(rename_all = "snake_case")]` 序列化为 `valid` / `invalid` / `unknown`，属跨端契约。区分"服务端明确失效"与"无法判定"至关重要：后者若按失效处理，会在赛前把选手踢回登录页，反复重登还可能触发 HOJ 的暴力破解锁定（同 IP + 同用户名 30 分钟 20 次）
- **`AuthService`** — 认证服务
  - `fn new(registry, session_repo, event_bus) -> Self` — 创建实例（**不持有 storage**；会话读写全部经 `session_repo`）
  - `async fn login(&self, username, password) -> AppResult<User>` — 调用 AuthProvider 登录 → **显式** `session_repo.save(&Session::new(...))` → 发布 `CoreEvent::LoggedIn { oj_id, user_id }`。**落盘失败则整体失败、不发布事件** —— 会话没落盘就宣告登录成功，会让重启后的客户端拿着不存在的会话工作。登录失败经 `e.context("登录失败")` 上抛，**保留原始变体**
  - `async fn logout(&self) -> AppResult<()>` — 远端登出（非致命，失败仅 `warn`）→ 显式删除本地会话 → 发布 `CoreEvent::LoggedOut { oj_id }`
  - `fn get_session(&self) -> Option<Session>` — 经 `session_repo.load(&oj_id)` 恢复会话；恢复成功时将 token 回注到 Provider（`AuthProvider::restore_token`），保证重启后认证请求仍携带 Authorization 头；仓库报错时 `warn` 留痕并按未登录处理（不静默吞掉 —— 否则「会话文件损坏」表现为「从未登录」）
  - `async fn validate_session(&self) -> SessionValidity` — 本地恢复 session（同时回注 token）→ 远端校验；无本地会话或服务端明确判定失效返回 `Invalid`（后者清除磁盘会话并发布 `CoreEvent::SessionExpired { oj_id }`），网络异常/无 Provider 返回 `Unknown`（保留本地会话）
  - `fn clear_session(&self, oj_id: &OjId) -> AppResult<()>`（私有）— 经 `session_repo.remove()` 删除会话（不存在时静默成功）
- **字段**：`registry: Arc<dyn ProviderRegistry>`, `session_repo: Arc<dyn SessionRepository>`, `event_bus: Arc<CoreEventBus>`

## 直接依赖
- `core::entity::session::Session`（re-export）
- `core::entity::user::User`
- `core::error::AppResult`（**不再直接引用 `AppError`**：`login` 传播 Provider 错误只用 `e.context(...)`，不构造新变体）
- `core::event::core_event::CoreEvent`
- `core::event::core_event_bus::CoreEventBus`
- `core::provider::oj_id::OjId`
- `core::provider::registry::ProviderRegistry`
- `core::repository::session_repo::SessionRepository`

## 被依赖
- `core::context`（`AppContext` 持有 `Arc<AuthService>`，装配时注入 `session_repo`）
- `commands::auth_cmd`（`login` / `logout` / `get_session` / `validate_session` 四个 Command 转发；并复用 `SessionValidity` 类型）

## 逻辑流程
- **login**：`registry.current_auth()` → `AuthProvider::login()`（失败 `warn!` + `e.context("登录失败")`，**变体原样穿透**）→ 构造 `Session::new(oj_id, user.id, user.username, user.token)` 并 `session_repo.save()`（失败即整体失败）→ 发布 `CoreEvent::LoggedIn { oj_id, user_id }`（**只带 OJ 与用户标识**：token 与完整 `User` 实体绝不进入事件流）
- **logout**：尝试远端 `logout()`（失败仅警告）→ `clear_session()` 删除 `sessions/{oj_id}.json` → 发布 `CoreEvent::LoggedOut { oj_id }`。用户域缓存（含源代码的提交详情/测试点）的清理由命令层显式编排（见 `commands/auth_cmd.md`）—— 同属「必须完成的核心清理」，不交给事件消费者
- **get_session**：`session_repo.load(&current_id())` 反序列化为 `Session`；恢复成功后调用 `registry.current_auth()` 获取 Provider 并回注 token；无会话或仓库报错返回 `None`
- **validate_session**：先 `get_session()`（无本地会话 → `Invalid`；有则顺带把 token 回注 Provider，否则重启后的校验请求必然 401）→ 取不到 AuthProvider → `Unknown`；调用远端 `validate_session()`：`Ok(true)` → `Valid`；`Ok(false)` → `clear_session()` 删除磁盘会话 + 发布 `CoreEvent::SessionExpired { oj_id }` → `Invalid`；`Err(_)`（网络异常）→ `Unknown` 并保留本地会话

## 设计要点
- **持久化职责的归属（本次重构的关键变更）**：会话落盘是「必须等待结果的核心动作」，因此**只在显式路径上完成** —— 登录时 `AuthService::login` 落盘、Provider 轮换时由适配器经 `SessionRepository` 落盘。旧实现让 Provider 发布带真实 token 的 `TokenRefreshed` 事件、由本服务的同步订阅者代劳写盘，既把凭证带进事件流，又把持久性保证挂在异步投递上（`subscribe_token_refresh` 已随旧事件系统一并删除）。
- **`login` 用 `context()` 保留变体**：密码错误（`Auth`）与服务器连不上（`Network`）对用户的处置完全不同 —— 前者要提示检查凭证，后者要提示检查网络/稍后重试。此前一律改写成 `AppError::Auth(format!("登录失败: {}", e))` 会让「服务器连不上」显示成「认证错误」，选手在赛前反复改密码重登，还可能撞上 HOJ 的暴力破解锁定（同 IP + 同用户名 30 分钟 20 次）。这也是全项目约定：Service 层传播 Provider 错误一律 `e.context("…")`，不得重新包装（见 `core/error.md`）。
- **三态契约由 Adapter 兑现，Service 只做映射**：`validate_session` 的 `Ok(true)`/`Ok(false)`/`Err(_)` → `Valid`/`Invalid`/`Unknown` 映射本身一直是正确的；`Err(_)` 分支能否被走到，取决于各 OJ Adapter 是否真的会在「无法判定」时返回 `Err`（HOJ 侧的判据见 `adapter/hoj/mod.md` 的 `session_validity_from_response`）。
- **登出清理与 OJ 切换清理都不走订阅**：`AuthService` 无任何订阅；提交详情/测试点缓存的清理由命令层（`auth_cmd::logout` / `oj_cmd::switch_oj`）显式调用。

## 测试
`src-tauri/src/service/auth/tests/auth_tests.rs` 锁定：会话保存/恢复 roundtrip（含 token 回注）、**登录显式落盘后才发布 `LoggedIn` 且载荷不含 token**、**落盘失败时不发布事件**（`login_does_not_publish_when_session_persist_fails`）、登出删除磁盘会话后才发布 `LoggedOut`、损坏会话文件返回 `None`、`clear_session` 幂等、`validate_session` 三态全覆盖（无本地会话/无 Provider → Invalid/Unknown、有效恢复 token、明确失效清会话并发事件、网络错误 Unknown 保留会话）、`SessionValidity` snake_case 序列化契约、**OjId 持久化契约**（`HOJAdapter::ID == "HOJ"` 且 `session_file() == "HOJ.json"` —— 内建 id 与历史枚举 Debug 输出不一致的话升级即静默丢全部既有会话）、**旧键名兼容**（以 `oj_type` 为键名的旧会话文件经 serde alias 反序列化到 `oj_id`）。桩 Provider 经 `registry.register(OjId, ProviderSet)` 部分能力注册（只挂 auth 一项）。

测试文件现**显式** `use crate::core::error::AppError;`：`mod.rs` 已不再直接引用 `AppError`，故 `use super::*` 不再把它带进作用域（`StubAuthProvider::validate_session` 的 `NetworkErr` 分支需要构造 `AppError::Network`）。

`StubAuthProvider` 以 `StubOutcome::{Valid, Invalid, NetworkErr}` 预设校验结果，对应的三态测试证明 **Service 侧映射一直是正确的** —— `validate_session` 三态失效的 bug 不在本模块，而在 HOJ adapter 此前把网络异常折成 `Ok(false)`、永不返回 `Err`，使 `Unknown` 分支对 HOJ 成为死代码（修复见 `adapter/hoj/mod.md`）。这组用例因此是「Service 契约」的回归护栏，与 adapter 侧的 `validity_*` 用例互补，两者都不可省。
