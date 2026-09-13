# http

## 职责
HTTP 客户端封装（基于 Reqwest），提供统一的超时、重试、UA、Cookie Store、认证头管理。

## 核心类型/函数
- **`HttpClient`** — HTTP 客户端 struct，封装 `reqwest::Client`
- **`HttpClient::new() -> Result<Self, reqwest::Error>`** — 创建默认客户端：30s 超时、Cookie Store、UA 为 `Hinina/{version}`
- **`HttpClient::client() -> &reqwest::Client`** — 获取内部 client 引用，供 Adapter 层直接调用原始 API
- **`HttpClient::get_json<T>(url, auth_token) -> AppResult<T>`** — GET + JSON 反序列化，5xx 自动重试，4xx 直接报错
- **`HttpClient::get_json_with_headers<T>(url, auth_token) -> AppResult<(T, HeaderMap)>`** — GET + JSON 反序列化，同时返回原始响应头（供 Adapter 层解析 OJ 私有头语义，如 HOJ token 轮换）
- **`HttpClient::post_json<T, B>(url, body, auth_token) -> AppResult<T>`** — POST JSON + 反序列化，不重试，4xx/5xx 直接报错
- **`HttpClient::post_json_with_headers<T, B>(url, body, auth_token) -> AppResult<(T, HeaderMap)>`** — POST JSON + 反序列化，同时返回原始响应头
- **`retry_get(url, auth_token) -> AppResult<Response>`** — 内部重试：GET 5xx 重试 2 次，指数退避 1s/2s，4xx 不重试
- **`status_error(url, status) -> AppError`** — HTTP 状态码 → AppError::Network 转换
- **`retry_delay(attempt) -> Duration`** — 计算重试延迟（指数退避）

## 直接依赖
- `reqwest::Client`
- `serde::{DeserializeOwned, Serialize}`
- `core::error::{AppError, AppResult}`
- `tokio::time::sleep`（重试延迟）
- `std::time::Duration`

## 被依赖
- `core::context`（`AppContext` 持有 `Arc<HttpClient>` 注入到各 Service）
- `adapter/hoj`（HOJ Adapter 将通过 HttpClient 发送 API 请求）

## 逻辑流程
创建时配置全局超时与 Cookie Store。GET 请求自动对 5xx 响应执行重试（最多 2 次，指数退避），4xx 客户端错误直接返回含状态码的错误。POST 为非幂等操作，任何非 2xx 状态码直接报错。通过 `auth_token` 参数可选附加 `Authorization` 请求头。`*_with_headers` 变体将响应头原样返回，infra 层不感知任何 OJ 私有协议语义（如 HOJ 的 `Refresh-Token` 轮换约定由 `adapter/hoj` 自行解析）。
