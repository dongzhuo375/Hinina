# http

## 职责
HTTP 客户端封装（基于 Reqwest），提供统一的超时、重试、UA、Cookie Store、认证头管理。

## 核心类型/函数
- **`HttpClient`** — HTTP 客户端 struct，封装 `reqwest::Client`
- **`HttpClient::new() -> Result<Self, reqwest::Error>`** — 创建默认客户端：30s 超时、Cookie Store、UA 为 `Hinina/{version}`
- **`HttpClient::client() -> &reqwest::Client`** — 获取内部 client 引用，供 Adapter 层直接调用原始 API
- **`HttpClient::get_json<T>(url, auth_token) -> AppResult<T>`** — GET + JSON 反序列化，自动重试 5xx
- **`HttpClient::post_json<T, B>(url, body, auth_token) -> AppResult<T>`** — POST JSON + 反序列化，不重试
- **`retry_get(url, auth_token) -> AppResult<Response>`** — 内部重试：GET 5xx 重试 2 次，指数退避 1s/2s

## 直接依赖
- `reqwest::Client`
- `serde::{DeserializeOwned, Serialize}`
- `core::error::AppResult`
- `tokio::time::sleep`（重试延迟）

## 被依赖
- `core::context`（`AppContext` 持有 `Arc<HttpClient>` 注入到各 Service）
- `adapter/hoj`（HOJ Adapter 将通过 HttpClient 发送 API 请求）

## 逻辑流程
创建时配置全局超时与 Cookie Store。GET 请求自动对 5xx 响应执行重试（最多 2 次），POST 为非幂等操作不重试。通过 `auth_token` 参数可选附加 `Authorization` 请求头。
