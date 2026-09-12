# auth

## 职责
定义认证 Provider trait `AuthProvider`，声明登录、登出、会话校验三个异步方法。各 OJ Adapter 需实现此 trait 以对接不同 OJ 的认证机制。

## 核心类型/函数
- **`AuthProvider`** — 认证 Provider trait（`#[async_trait]`），方法：
  - `login(&self, username, password) -> AppResult<User>` — 登录
  - `logout(&self) -> AppResult<()>` — 登出
  - `validate_session(&self) -> AppResult<bool>` — 校验会话有效性
  - `restore_token(&self, token: &str)` — 恢复本地会话 token（应用重启后由 `AuthService::get_session()` 回注，保证后续认证请求携带 Authorization 头）

## 直接依赖
- `async_trait::async_trait`
- `core::entity::user::User`
- `core::error::AppResult`

## 被依赖
- `core::provider::registry`（ProviderRegistry 注册/获取 AuthProvider）
- `infra::provider_registry_impl`

## 逻辑流程
无（纯 trait 定义）。
