# http

## 职责
HTTP 客户端封装（基于 Reqwest），提供统一的超时、重试、UA、Cookie Store。**请求头由调用方注入**（`HeaderMap` 原样附加）：不同 OJ 的凭证形态各异（HOJ 的 JWT 走 `Authorization` 头、Hydro 走 Cookie 会话），认证方式是 Adapter 层概念，infra 不做任何假设。

**只负责传输，不做反序列化**：对外方法一律返回「原始响应体文本 + 响应头」，不感知任何 OJ 私有的响应约定。JSON 解析、字段归一化（如 HOJ 的剔除 `null`）、响应头私有语义（如 HOJ 的 token 轮换）全部由 Adapter 层承担。

**但承担 HTTP 通用语义的状态码判定**：`status_error` 把 HTTP 401 映射为 `AppError::Auth`、其余错误状态码映射为 `Network`（详见「核心类型/函数」与「逻辑流程」）。这是「通用语义 vs OJ 私有约定」的分界线，不是对上一条的例外。

## 核心类型/函数
- **`HttpClient`** — HTTP 客户端 struct，封装 `reqwest::Client`
- **`HttpClient::new() -> Result<Self, reqwest::Error>`** — 创建默认客户端：30s 超时、Cookie Store、UA 为 `Hinina/{version}`（内部委托 `with_timeout(30s)`）
- **`HttpClient::with_timeout(timeout: Duration) -> Result<Self, reqwest::Error>`** — 创建指定超时的客户端。**超时来自 `oj.timeout_secs` 配置**（由 `core/context.rs` 装配时读取并注入，`timeout_secs.max(1)` 防 0 值），不再硬编码；其余行为（Cookie Store、UA、重试）与 `new()` 一致
- **`HttpClient::client() -> &reqwest::Client`** — 获取内部 client 引用，供 Adapter 层直接调用原始 API（HOJ 的 `login` 需自行读响应头取 token、`logout` 忽略响应体）。**`validate_session` 已不再走 raw client**：它改走 `get_json_authed`，才能拿到去 null 解析、体内鉴权失败识别、token 轮换与 5xx 退避重试（见 `adapter/hoj/mod.md`）
- **`HttpClient::get_text_with_headers(url, headers) -> AppResult<(String, HeaderMap)>`** — GET，返回**原始响应体**与响应头；5xx 与传输错误自动重试，4xx 直接报错；`headers` 由调用方注入并原样附加（空 map = 无附加头，重试时原样重附）
- **`HttpClient::post_text_with_headers<B: Serialize>(url, body, headers) -> AppResult<(String, HeaderMap)>`** — POST（JSON body），返回**原始响应体**与响应头；非幂等，不重试，非 2xx 直接报错
- **`retry_get(url, headers) -> AppResult<Response>`**（私有）— 内部重试：最多 2 次，指数退避 1s/2s；4xx 立即报错不重试；**5xx 重试耗尽后同样报错**（处置判据见 `classify_status`）
- **`classify_status(status, attempt) -> StatusDecision`**（私有纯函数）— GET 的处置判据：`Accept`（2xx）/ `Retry`（5xx 且 `attempt < MAX_RETRIES`）/ `Fail`（其余非成功状态，含重试耗尽的 5xx 与 3xx 残留）。抽成纯函数是为了让判据可被单元测试穷尽锁定 —— 这里曾有 bug：5xx 耗尽后落到 `return Ok(response)`，把网关 HTML 错误页当成正常响应，最终报成「响应不是合法 JSON」而不是「HTTP 502」，把排障引向错误方向，同时使 `retry_get` 末尾的 `Err(last_error)` 成为永不可达的死代码
- **`StatusDecision`**（私有枚举）— `Accept` / `Retry` / `Fail`
- **`status_error(url, status) -> AppError`**（私有）— HTTP 状态码 → `AppError`，消息统一为 `"HTTP {code} {reason}: {url}"`。**401 → `Auth`，其余（含 403 与全部 5xx）→ `Network`**
  - 401 单独映射的理由：HTTP 401 的标准语义就是「未认证」，与会话失效等价；而前端 `sessionGuard` 与 `AuthService::validate_session` 都**只依据 `Auth` 变体**判定失效。若一律归 `Network`，token 过期时守卫不会触发 —— 选手只会看到「网络错误」，永远回不到登录页。这属 HTTP 通用语义而非 OJ 私有约定，故由 infra 层承担；OJ 把鉴权失败藏在响应体（HTTP 200 + 体内 `status=401/403`）的情形由 Adapter 层识别（`adapter/hoj` 的 `auth_failure_from_body`）
  - 这条映射是 HOJ 会话校验能成立的**前提**：`get_json_authed` 遇到 HTTP 401 时若仍归 `Network`，`HOJAdapter::session_validity_from_response` 会把它当「无法判定」上抛 → `SessionValidity::Unknown` → **token 真正过期时反而永不登出**
  - **403 刻意保持 `Network`**：它可能是「无权访问某场私有赛」这类业务限制而非会话问题，误判为 `Auth` 会把已登录选手踢回登录页
- **`retry_delay(attempt) -> Duration`**（私有 const）— 指数退避延迟 `RETRY_BASE_DELAY_MS * 2^attempt`
- **常量 `MAX_RETRIES = 2` / `RETRY_BASE_DELAY_MS = 1000`** — 重试次数与退避基数

> 已移除：`get_json` / `get_json_with_headers` / `post_json` / `post_json_with_headers`。这些方法在 infra 内部调用 `serde_json::from_str`，使 Adapter 无法在「拿到原始 JSON」与「类型化解析」之间插入 OJ 特有的归一化步骤（HOJ 必须先剔除 `null`、再判定响应体内的鉴权失败），因此整体下沉为 `*_text_with_headers`。

## 直接依赖
- `reqwest::Client`
- `serde::Serialize`（POST body 序列化；已不再需要 `DeserializeOwned`，因为本层不做反序列化）
- `core::error::{AppError, AppResult}`
- `tokio::time::sleep`（重试延迟）
- `std::time::Duration`

## 被依赖
- `core::context`（`AppContext` 持有 `Arc<HttpClient>` 注入到各 Service）
- `adapter/hoj`（HOJ Adapter 将通过 HttpClient 发送 API 请求）

## 逻辑流程
创建时配置全局超时（`with_timeout` 由配置驱动，见上）与 Cookie Store。GET 请求自动对 5xx 响应执行重试（最多 2 次，指数退避），4xx 客户端错误直接返回含状态码的错误。POST 为非幂等操作，任何非 2xx 状态码直接报错。请求头经 `headers` 参数由调用方注入（认证方式是 Adapter 层概念，infra 不感知——HOJ 的 `Authorization` 头由 `adapter/hoj` 的 `auth_headers` 组装）。`*_with_headers` 变体将响应头原样返回，infra 层不感知任何 OJ 私有协议语义（如 HOJ 的 `Refresh-Token` 轮换约定由 `adapter/hoj` 自行解析）。

**状态码 → 错误变体映射**（GET 与 POST 共用 `status_error`）：

| 响应 | GET（`retry_get`） | POST |
|------|------|------|
| 2xx | `Ok(原始响应体, 响应头)` | 同左 |
| 401 | `Err(Auth("HTTP 401 Unauthorized: {url}"))`，**不重试** | 同左 |
| 其它 4xx（含 403） | `Err(Network("HTTP {code} {reason}: {url}"))`，**不重试**（客户端错误重试无意义） | 同左 |
| 5xx | 退避 1s / 2s 后重试，最多 2 次；**重试耗尽后 `Err(Network("HTTP {code} …"))`**，不会把网关错误页当成正常响应交给上层 | 不重试，直接 `Err(Network(...))` |
| 传输错误 / 超时 | 同样退避重试 2 次；耗尽后 `Err(Network("GET 请求失败 {url}: {e}"))` | `Err(Network("POST 请求失败 {url}: {e}"))` |
| 读响应体失败 | `Err(Network("读取响应体失败: {e}"))` | 同左 |

> 变体是前端 `isAuthError` 分流与 `sessionGuard` 会话失效兜底的唯一依据，因此本层**只在传输环节构造错误，绝不改写下游变体**；Adapter 与 Service 补环节名一律用 `AppError::context()`（见 `core/error.md`）。

## 测试
`src-tauri/src/infra/tests/http_tests.rs`（由 `http.rs` 底部 `#[cfg(test)] #[path = "tests/http_tests.rs"] mod tests;` 引用）锁定：`status_error` 把 **HTTP 401 映射为 `Auth`** 变体（并断言消息保留状态码与 URL，便于现场排障）、**403 保持 `Network`**（业务性无权访问不得误判为会话失效）、400 / 404 / 500 / 502 / 503 一律 `Network`、`retry_delay` 指数退避 1s / 2s / 4s（纯函数性质；实际重试只用到 attempt 0/1，即 1s / 2s）；`classify_status` 的完整判据 —— 2xx 在任意 attempt 都 `Accept`、5xx（500/502/503/504）在额度内 `Retry` 而**耗尽后 `Fail`**（正面锁定上述回归）、4xx（400/401/403/404）在任意 attempt 都 `Fail` 不重试、3xx 残留同样 `Fail`；`with_timeout` 构造路径可用（reqwest::Client 不暴露 timeout getter，「配置值确实被传入」由 context.rs 的装配代码保证：`timeout_secs → with_timeout`）。
