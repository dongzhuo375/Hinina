# auth_cmd

## 职责
认证相关 Tauri Command 模块。面向前端暴露登录、登出、会话查询与会话有效性校验四个 IPC 命令，均为 `AuthService` 的薄封装（不含业务逻辑）。

## 核心类型/函数
- `pub fn parse_oj_type(s: &str) -> Option<OJType>` — 字符串解析 OJType（大小写不敏感，未知值返回 `None`）；公开以支持 Command 层测试
- `pub async fn login(ctx, username, password, oj_type) -> AppResult<User>` — 登录；传入 `oj_type` 时先切换 `ProviderRegistry` 当前 OJ，未知值告警并回退默认 OJ
- `pub async fn logout(ctx) -> AppResult<()>` — 登出当前会话；**编排两件事**：先 `AuthService::logout()`（其内部即使远端登出失败也返回 Ok），再 `SubmissionService::clear_user_caches()` 清空用户域缓存（终态提交详情/测试点，含源代码）—— 缓存是内存态，不清理会让同机换账号后仍能读到上一位选手的提交内容；`AuthService` 的错误原样返回（清缓存不因登出失败而跳过）
- `pub async fn get_session(ctx) -> AppResult<Option<User>>` — 查询本地保存的会话（`None` = 从未登录或已登出）
- `pub async fn validate_session(ctx) -> AppResult<SessionValidity>` — 三态校验会话有效性（`valid` / `invalid` / `unknown`）

## 直接依赖
- `tauri::State`
- `crate::core::context::AppContext`
- `crate::core::entity::user::User`
- `crate::core::error::AppResult`
- `crate::core::provider::oj_type::OJType`
- `crate::service::auth::SessionValidity`

## 被依赖
- `src-tauri/src/commands/mod.rs`（`pub mod auth_cmd` 声明）
- `src-tauri/src/main.rs`（`tauri::generate_handler!` 注册四个命令）
- 前端 `src/bridge/auth.bridge.ts`（`login` / `logout` / `get_session` / `validate_session`）

## 逻辑流程
1. 前端 `invoke('login' | 'logout' | 'get_session' | 'validate_session', ...)`
2. Command 从 `State<AppContext>` 取出 `Arc<AuthService>` 并转发调用
3. 错误以 `AppError` 返回，经 serde 序列化为 `{ Variant: msg }`；前端在 `bridge/index.ts` 归一化为 `IpcError`

`validate_session` 的三态语义是跨端契约：
- `invalid` — 本地无会话，或服务端已判定失效（`AuthService` 已清除磁盘会话并发布 `SessionExpired`），前端须回到登录页
- `unknown` — 网络异常 / Provider 不可用，本地会话保留，前端应稍后重试而**不是**踢出用户（赛前误踢的代价远高于多等一轮校验）
