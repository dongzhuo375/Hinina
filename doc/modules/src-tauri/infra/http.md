# http

## 职责
HTTP 客户端封装（基于 Reqwest），提供统一的超时、重试、UA、Cookie Store、认证头管理。

**只负责传输，不做反序列化**：对外方法一律返回「原始响应体文本 + 响应头」，不感知任何 OJ 私有的响应约定。JSON 解析、字段归一化（如 HOJ 的剔除 `null`）、响应头私有语义（如 HOJ 的 token 轮换）全部由 Adapter 层承担。

## 核心类型/函数
- **`HttpClient`** — HTTP 客户端 struct，封装 `reqwest::Client`
- **`HttpClient::new() -> Result<Self, reqwest::Error>`** — 创建默认客户端：30s 超时、Cookie Store、UA 为 `Hinina/{version}`
- **`HttpClient::client() -> &reqwest::Client`** — 获取内部 client 引用，供 Adapter 层直接调用原始 API（HOJ 的 `login` / `logout` / `validate_session` 需要自行读响应头或忽略响应体）
- **`HttpClient::get_text_with_headers(url, auth_token) -> AppResult<(String, HeaderMap)>`** — GET，返回**原始响应体**与响应头；5xx 与传输错误自动重试，4xx 直接报错
- **`HttpClient::post_text_with_headers<B: Serialize>(url, body, auth_token) -> AppResult<(String, HeaderMap)>`** — POST（JSON body），返回**原始响应体**与响应头；非幂等，不重试，非 2xx 直接报错
- **`retry_get(url, auth_token) -> AppResult<Response>`**（私有）— 内部重试：最多 2 次，指数退避 1s/2s；4xx 立即返回错误
- **`status_error(url, status) -> AppError`**（私有）— HTTP 状态码 → `AppError::Network("HTTP {code} {reason}: {url}")`
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
创建时配置全局超时与 Cookie Store。GET 请求自动对 5xx 响应执行重试（最多 2 次，指数退避），4xx 客户端错误直接返回含状态码的错误。POST 为非幂等操作，任何非 2xx 状态码直接报错。通过 `auth_token` 参数可选附加 `Authorization` 请求头。`*_with_headers` 变体将响应头原样返回，infra 层不感知任何 OJ 私有协议语义（如 HOJ 的 `Refresh-Token` 轮换约定由 `adapter/hoj` 自行解析）。
