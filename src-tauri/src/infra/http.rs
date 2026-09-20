use std::time::Duration;

use serde::Serialize;

use crate::core::error::AppResult;

/// HTTP 客户端封装（基于 Reqwest）。
///
/// 提供统一的超时、重试、UA、Cookie Store。
///
/// **只返回原始响应体，不做反序列化**：各 OJ 的响应往往需要协议特定的归一化
/// （例如 HOJ 对未设置字段返回 `null`，必须先剔除才能喂给 serde），这类语义属于
/// Adapter 层；infra 只负责传输、状态码判定与重试，不感知任何 OJ 私有约定。
///
/// **请求头由调用方注入**（`HeaderMap` 原样附加）：不同 OJ 的凭证形态各异
/// （HOJ 的 JWT 走 `Authorization` 头、Hydro 走 Cookie 会话、有的 OJ 还要
/// CSRF 令牌）—— 认证方式是 Adapter 层概念，infra 不做任何假设。
///
/// **两组变体，按「是否需要自己读非 2xx 的响应体」选**：
/// - `*_with_headers`：非 2xx 直接映射为 `AppError`（401 → `Auth`），
///   **并把响应体摘录附进错误信息**（见 `read_error_excerpt`）—— 排查「服务端到底
///   说了什么」全靠它；
/// - `*_raw`：任意状态码都返回 `(status, headers, body)`，**不做状态码映射** ——
///   供把错误信息放在响应体里的 OJ 使用（Hydro 的 `{"error":{…}}` 是用户可见
///   文案的唯一来源），代价是调用方需自行映射 401 → `Auth`。
pub struct HttpClient {
    client: reqwest::Client,
}

/// 重试配置
const MAX_RETRIES: u32 = 2;
const RETRY_BASE_DELAY_MS: u64 = 1000;

/// 计算第 `attempt` 次重试的延迟时间（指数退避）。
const fn retry_delay(attempt: u32) -> Duration {
    Duration::from_millis(RETRY_BASE_DELAY_MS * 2u64.pow(attempt))
}

/// 将 HTTP 状态码转为 AppError。
///
/// **401 单独映射为 `Auth`**：HTTP 401 的标准语义就是「未认证」，与会话失效等价。
/// 前端 `sessionGuard` 与 `AuthService::validate_session` 都依据 `Auth` 变体判定失效，
/// 若一律归为 `Network`，token 过期时守卫不会触发 —— 选手只会看到「网络错误」，
/// 永远回不到登录页。这是 HTTP 通用语义而非 OJ 私有约定，故由 infra 层承担；
/// OJ 把鉴权失败藏在响应体（HTTP 200 + body status=403）的情形由 Adapter 层识别。
///
/// **403 保持 `Network`**：它可能是「无权访问某场私有赛」这类业务限制而非会话问题，
/// 误判为 Auth 会把已登录选手踢回登录页。
fn status_error(url: &str, status: reqwest::StatusCode) -> crate::core::error::AppError {
    let msg = format!("HTTP {} {}: {}", status.as_u16(), status.canonical_reason().unwrap_or(""), url);
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return crate::core::error::AppError::Auth(msg);
    }
    crate::core::error::AppError::Network(msg)
}

/// 错误响应体摘录的最大字符数。
const ERROR_BODY_EXCERPT_LIMIT: usize = 300;

/// 读取非 2xx 响应的响应体，压成单行并截断，供错误信息附带。
///
/// **存在理由**：许多 OJ 把「为什么失败」放在响应体里（HOJ 的 500 会带
/// `{"status":500,"msg":"…"}`，Hydro 带 `{"error":{…}}`）。非 raw 变体此前直接丢弃
/// 响应体，调用方只能拿到一句 `HTTP 500 Internal Server Error` —— 实测排查 HOJ
/// 提交失败时，日志里十条一模一样的「HTTP 500」，完全看不出服务端说了什么。
///
/// 读取失败（连接中断等）降级为空串：错误信息本身不该因为「读错误信息失败」而丢失。
async fn read_error_excerpt(response: reqwest::Response) -> String {
    let Ok(text) = response.text().await else {
        return String::new();
    };
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= ERROR_BODY_EXCERPT_LIMIT {
        return one_line;
    }
    let truncated: String = one_line.chars().take(ERROR_BODY_EXCERPT_LIMIT).collect();
    format!("{}…", truncated)
}

/// 带响应体摘录的状态码错误。
///
/// **401 不附带响应体**：该变体是前端 `sessionGuard` 的判据，且服务端在 401 响应里
/// 可能回显请求凭证 —— 认证失败的原因由 HTTP 语义本身说明，无需正文。
fn status_error_with_body(
    url: &str,
    status: reqwest::StatusCode,
    excerpt: &str,
) -> crate::core::error::AppError {
    if status == reqwest::StatusCode::UNAUTHORIZED || excerpt.is_empty() {
        return status_error(url, status);
    }
    let msg = format!(
        "HTTP {} {}: {} | {}",
        status.as_u16(),
        status.canonical_reason().unwrap_or(""),
        url,
        excerpt
    );
    crate::core::error::AppError::Network(msg)
}

/// GET 响应的处置决策。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusDecision {
    /// 2xx（或 raw 变体下的任意状态码）：交给调用方读取响应体
    Accept,
    /// 5xx 且仍有重试额度：退避后重试
    Retry,
    /// 其余非成功状态（4xx、以及重试耗尽的 5xx）：立即报错
    Fail,
}

/// 按状态码、当前尝试次数与「是否允许错误状态码」决定处置方式。
///
/// 抽成纯函数是为了让「哪些状态码重试、哪些立即失败」可被单元测试穷尽锁定 ——
/// 这里曾经有个 bug：5xx 在重试耗尽后落到 `return Ok(response)`，
/// 把网关的 HTML 错误页当成正常响应交给上层，最终报成「响应不是合法 JSON」
/// 而不是「HTTP 502」，把排障引向错误方向；同时末尾的 `Err(last_error)` 成了死代码。
///
/// `allow_error` 由 raw 变体传入（`get_text_raw` / `post_text_raw`）：**任意状态码
/// 都返回原始响应**，因为有些 OJ 把错误信息放在响应体里，而响应体在非 raw 变体里
/// 会被丢弃。5xx 仍然重试（GET 幂等），只是**重试耗尽后返回响应而不是报错**。
fn classify_status(status: reqwest::StatusCode, attempt: u32, allow_error: bool) -> StatusDecision {
    if status.is_server_error() {
        return if attempt < MAX_RETRIES {
            StatusDecision::Retry
        } else if allow_error {
            StatusDecision::Accept
        } else {
            StatusDecision::Fail
        };
    }
    if status.is_success() {
        return StatusDecision::Accept;
    }
    // 4xx（含 401/403）与 3xx 等其它非成功状态：重试同样的请求只会得到同样的结果
    if allow_error {
        StatusDecision::Accept
    } else {
        StatusDecision::Fail
    }
}

impl HttpClient {
    /// 创建默认 HttpClient：30 秒超时、Cookie Store 已启用、UA 为 Hinina/{version}。
    pub fn new() -> Result<Self, reqwest::Error> {
        Self::with_timeout(Duration::from_secs(30))
    }

    /// 创建指定超时的 HttpClient（超时来自 `oj.timeout_secs` 配置）。
    pub fn with_timeout(timeout: Duration) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .cookie_store(true)
            .user_agent(format!("Hinina/{}", env!("CARGO_PKG_VERSION")))
            .timeout(timeout)
            .build()?;
        Ok(Self { client })
    }

    /// 获取内部 reqwest::Client 引用（供 Adapter 层直接调用原始 API）。
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    // ── 请求方法 ──

    /// 发送 GET 请求，返回**原始响应体**与响应头（自动重试 5xx）。
    ///
    /// `headers` 由调用方注入并**原样附加**（认证方式是 Adapter 层概念，
    /// infra 不感知 —— 空 map 即无附加头）。
    /// 对 4xx 错误直接返回 `AppError::Network`（含状态码），不重试。
    /// 响应头原样暴露给调用方，用于解析协议特定的头语义（如 HOJ 的 token 轮换）。
    pub async fn get_text_with_headers(
        &self,
        url: &str,
        headers: &reqwest::header::HeaderMap,
    ) -> AppResult<(String, reqwest::header::HeaderMap)> {
        let response = self.retry_get(url, headers, false).await?;
        let response_headers = response.headers().clone();
        let body = response.text().await.map_err(|e| {
            crate::core::error::AppError::Network(format!("读取响应体失败: {}", e))
        })?;
        Ok((body, response_headers))
    }

    /// 发送 GET 请求，**任意状态码都返回原始响应**（状态码 + 响应头 + 响应体）。
    ///
    /// 与 [`HttpClient::get_text_with_headers`] 的唯一区别是**不做状态码 → `AppError`
    /// 映射**。存在理由：有些 OJ 把错误信息放在响应体里（如 Hydro 的
    /// `{"error":{"name","params","code"}}` 是用户可见文案的唯一来源），而 4xx/5xx 的
    /// 响应体在非 raw 变体里会被丢弃，调用方只能得到一句「HTTP 403」。
    ///
    /// 重试策略：5xx 仍按退避重试（GET 幂等），**重试耗尽后把最后一次响应原样返回**；
    /// 4xx 不重试（重试只会得到同样的结果）。
    ///
    /// **调用方需自行处理状态码语义**：401 不会自动变成 `Auth` 变体，而前端
    /// `sessionGuard` 的会话失效判定依赖该变体 —— 用本方法就必须自己映射
    /// （判据见 `status_error`，Adapter 侧有同判据的镜像实现）。
    pub async fn get_text_raw(
        &self,
        url: &str,
        headers: &reqwest::header::HeaderMap,
    ) -> AppResult<(reqwest::StatusCode, reqwest::header::HeaderMap, String)> {
        let response = self.retry_get(url, headers, true).await?;
        let status = response.status();
        let response_headers = response.headers().clone();
        let body = response.text().await.map_err(|e| {
            crate::core::error::AppError::Network(format!("读取响应体失败: {}", e))
        })?;
        Ok((status, response_headers, body))
    }

    /// 发送 POST 请求（JSON body），返回**原始响应体**与响应头。
    ///
    /// `headers` 语义同 [`HttpClient::get_text_with_headers`]。
    /// POST 为非幂等方法，不执行自动重试。
    /// 对 4xx/5xx 错误直接返回 `AppError::Network`（含状态码与响应体摘录）。
    pub async fn post_text_with_headers<B: Serialize>(
        &self,
        url: &str,
        body: &B,
        headers: &reqwest::header::HeaderMap,
    ) -> AppResult<(String, reqwest::header::HeaderMap)> {
        let req = self.client.post(url).json(body).headers(headers.clone());
        let response = req.send().await.map_err(|e| {
            crate::core::error::AppError::Network(format!("POST 请求失败 {}: {}", url, e))
        })?;
        if !response.status().is_success() {
            let status = response.status();
            let excerpt = read_error_excerpt(response).await;
            return Err(status_error_with_body(url, status, &excerpt));
        }
        let response_headers = response.headers().clone();
        let text = response.text().await.map_err(|e| {
            crate::core::error::AppError::Network(format!("读取响应体失败: {}", e))
        })?;
        Ok((text, response_headers))
    }

    /// 发送 POST 请求（JSON body），**任意状态码都返回原始响应**。
    ///
    /// 语义同 [`HttpClient::get_text_raw`]（不做状态码映射，供需要读错误包络的
    /// 适配器使用），差别是 POST 为非幂等方法、**不做任何重试**。
    pub async fn post_text_raw<B: Serialize>(
        &self,
        url: &str,
        body: &B,
        headers: &reqwest::header::HeaderMap,
    ) -> AppResult<(reqwest::StatusCode, reqwest::header::HeaderMap, String)> {
        let response = self
            .client
            .post(url)
            .json(body)
            .headers(headers.clone())
            .send()
            .await
            .map_err(|e| {
                crate::core::error::AppError::Network(format!("POST 请求失败 {}: {}", url, e))
            })?;
        let status = response.status();
        let response_headers = response.headers().clone();
        let text = response.text().await.map_err(|e| {
            crate::core::error::AppError::Network(format!("读取响应体失败: {}", e))
        })?;
        Ok((status, response_headers, text))
    }

    // ── 内部重试逻辑 ──

    /// 发送 GET 请求，对 5xx 与传输错误自动重试（最多 2 次，指数退避 1s/2s）。
    ///
    /// 4xx 直接报错不重试（客户端错误重试只会得到同样的结果）；
    /// **5xx 在重试耗尽后**：`allow_error` 为假时报错（不把错误页当正常响应交给上层
    /// 解析），为真时返回最后一次响应让调用方自己读状态码与响应体
    /// （处置判据见 `classify_status`）。请求头每次重试原样重附。
    async fn retry_get(
        &self,
        url: &str,
        headers: &reqwest::header::HeaderMap,
        allow_error: bool,
    ) -> AppResult<reqwest::Response> {
        let mut last_error: Option<crate::core::error::AppError> = None;

        for attempt in 0..=MAX_RETRIES {
            let req = self.client.get(url).headers(headers.clone());

            match req.send().await {
                Ok(response) => {
                    let status = response.status();
                    match classify_status(status, attempt, allow_error) {
                        StatusDecision::Accept => return Ok(response),
                        StatusDecision::Fail => {
                            let excerpt = read_error_excerpt(response).await;
                            return Err(status_error_with_body(url, status, &excerpt));
                        }
                        StatusDecision::Retry => {
                            let delay = retry_delay(attempt);
                            tokio::time::sleep(delay).await;
                            last_error = Some(crate::core::error::AppError::Network(format!(
                                "服务端错误 {} (status: {})，第 {} 次重试",
                                url,
                                status,
                                attempt + 1
                            )));
                            continue;
                        }
                    }
                }
                Err(e) => {
                    if attempt < MAX_RETRIES {
                        let delay = retry_delay(attempt);
                        tokio::time::sleep(delay).await;
                        last_error = Some(crate::core::error::AppError::Network(format!(
                            "请求失败 {}: {}，第 {} 次重试",
                            url,
                            e,
                            attempt + 1
                        )));
                        continue;
                    }
                    return Err(crate::core::error::AppError::Network(format!(
                        "GET 请求失败 {}: {}",
                        url, e
                    )));
                }
            }
        }

        // 防御性兜底：循环体内每条路径都已 return（Retry 仅在 attempt < MAX_RETRIES 时发生），
        // 正常不会走到这里；保留它是为了让「循环意外退出」也返回带上下文的错误而不是 panic
        Err(last_error.unwrap_or_else(|| {
            crate::core::error::AppError::Network(format!("GET 请求失败（已达最大重试）: {}", url))
        }))
    }
}

#[cfg(test)]
#[path = "tests/http_tests.rs"]
mod tests;
