# error

## 职责
Hydro 适配器专用错误类型，以及「错误包络 → `AppError`」的变体判定。与 `adapter/hoj/error.rs` 同构，但判据更精确：HOJ 的错误只带数字状态码，Hydro 的错误带**机器可读的错误名**。

## 核心类型/函数
- `HydroError` — 适配器内部错误枚举：
  - `ApiError { code, name, params }` — 响应体 `{"error":{...}}`（**没有 message**，文案必须由调用方组织）
  - `HttpError(String)` — HTTP 传输失败
  - `JsonError(String)` — 解析失败
  - `Unauthorized(String)` — 未认证 / 会话失效
  - `UnknownStatus(i64)` — 评测状态码不在码表内
- `is_auth_error_name(name) -> bool` — 判断错误名是否属于「会话失效」。只认 `PrivilegeError`（未登录，`params` 带所需 PRIV 常量）/ `LoginError` / `BuiltinLoginError`（登录失败）。**`PermissionError` 不算**（已登录但无权限，如未报名私有赛）—— 误判成会话失效会把已登录选手踢回登录页，这是赛场上最坏的失败方式；`CsrfTokenError`（Referer 校验）/ `OpcountExceededError`（限流）/ `BlacklistedError`（IP 黑名单）同理
- `classify_business_error(name, code, detail) -> AppError`（私有）— 按错误名的领域前缀（`Problem*` / `Contest*`·`Homework*` / `Record*`）归入既有的 `Problem` / `Contest` / `Submission` 变体，与 HOJ 侧的分类粒度对齐。**尽力而为的启发式**：Hydro 未公开完整错误名清单，未命中前缀一律落 `Unknown`（不猜领域 —— 变体是前端展示分流依据，猜错比不分类更糟；控制流只依赖 `Auth`）
- `impl From<HydroError> for AppError` — 变体翻译。会话失效**优先于**领域归类（`PrivilegeError` 也带 403，但语义是「未登录」）

## 直接依赖
- `thiserror::Error`
- `core::error::AppError`

## 被依赖
- `adapter::hydro::types::parse_hydro_error` — 错误包络 → `AppError`
- `adapter::hydro::mod` — `HttpResponse::into_value()` 的错误分支、`validate_session` 的 `Err(Auth)` 判定

## 逻辑流程
```
HydroError::ApiError{code,name,params}
   ├─ is_auth_error_name(name) ─► AppError::Auth          （前端据此清会话回登录页）
   └─ 否则按名前缀 ─────────────► Problem / Contest / Submission / Unknown
HydroError::HttpError ──────────► AppError::Network
HydroError::JsonError ──────────► AppError::Serialization
HydroError::Unauthorized ───────► AppError::Auth
HydroError::UnknownStatus ──────► AppError::Unknown
```

## 测试
`error.rs` 内联 `#[cfg(test)] mod tests`（与 HOJ 侧 error 的测试组织一致）：
- `PrivilegeError` → `Auth`
- `PermissionError` / `CsrfTokenError` / `OpcountExceededError` / `BlacklistedError` **不**触发登出
- 领域前缀归类（`ProblemNotFoundError` / `ContestNotLiveError` / `RecordNotFoundError` / `HomeworkNotFoundError`）
- 未知名回落 `Unknown`（不猜领域）
- 传输/解析/未授权错误保持各自变体
