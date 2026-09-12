# http

## 职责
HTTP 客户端封装（基于 Reqwest），提供统一的超时、重试、UA、Cookie Store、认证头管理。

## 核心类型/函数
- **`HttpClient`** — HTTP 客户端 struct，封装 `reqwest::Client`
- **`HttpClient::new() -> Result<Self, reqwest::Error>`** — 创建默认客户端：30s 超时、Cookie Store、UA 为 `Hinina/{version}`
- **`HttpClient::client() -> &reqwest::Client`** — 获取内部 client 引用，供 Adapter 层直接调用原始 API
- **`HttpClient::get_json<T>(url, auth_token) -> AppResult<T>`** — GET + JSON 反序列化，5xx 自动重试，4xx 直接报错
- **`HttpClient::get_json_with_refresh<T>(url, auth_token) -> AppResult<(T, Option<String>)>`** — GET + JSON 反序列化，同时检测服务端 token 轮换（`Refresh-Token` 头），返回 `(解析数据, 轮换后的新 token)`
- **`HttpClient::post_json<T, B>(url, body, auth_token) -> AppResult<T>`** — POST JSON + 反序列化，不重试，4xx/5xx 直接报错
- **`retry_get(url, auth_token) -> AppResult<Response>`** — 内部重试：GET 5xx 重试 2 次，指数退避 1s/2s，4xx 不重试
- **`status_error(url, status) -> AppError`** — HTTP 状态码 → AppError::Network 转换
- **`retry_delay(attempt) -> Duration`** — 计算重试延迟（指数退避）
- **`extract_refreshed_token(response) -> Option<String>`** — 从响应头检测 token 轮换（存在 `Refresh-Token` 头时提取新 `Authorization` 头）

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
创建时配置全局超时与 Cookie Store。GET 请求自动对 5xx 响应执行重试（最多 2 次，指数退避），4xx 客户端错误直接返回含状态码的错误。POST 为非幂等操作，任何非 2xx 状态码直接报错。通过 `auth_token` 参数可选附加 `Authorization` 请求头。
