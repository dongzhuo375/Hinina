use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::core::error::AppResult;

/// HTTP 客户端封装（基于 Reqwest）。
///
/// 提供统一的超时、重试、UA、Cookie Store、认证头管理。
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

/// 从响应头检测 token 轮换，返回轮换后的新 token（未轮换返回 `None`）。
fn extract_refreshed_token(response: &reqwest::Response) -> Option<String> {
    if response.headers().get("refresh-token").is_some() {
        response
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
    } else {
        None
    }
}

/// 将 HTTP 状态码转为 AppError
fn status_error(url: &str, status: reqwest::StatusCode) -> crate::core::error::AppError {
    let msg = format!("HTTP {} {}: {}", status.as_u16(), status.canonical_reason().unwrap_or(""), url);
    crate::core::error::AppError::Network(msg)
}

impl HttpClient {
    /// 创建默认 HttpClient：30 秒超时、Cookie Store 已启用、UA 为 Hinina/{version}。
    pub fn new() -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .cookie_store(true)
            .user_agent(format!("Hinina/{}", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self { client })
    }

    /// 获取内部 reqwest::Client 引用（供 Adapter 层直接调用原始 API）。
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    // ── 便捷请求方法 ──

    /// 发送 GET 请求并反序列化 JSON 响应体（自动重试 5xx）。
    ///
    /// `auth_token` 为 `Some` 时自动附加 `Authorization` 头。
    /// 对 4xx 错误直接返回 `AppError::Network`（含状态码），不重试。
    pub async fn get_json<T: DeserializeOwned>(
        &self,
        url: &str,
        auth_token: Option<&str>,
    ) -> AppResult<T> {
        let response = self.retry_get(url, auth_token).await?;
        let body = response.text().await.map_err(|e| {
            crate::core::error::AppError::Network(format!("读取响应体失败: {}", e))
        })?;
        serde_json::from_str::<T>(&body).map_err(|e| {
            crate::core::error::AppError::Serialization(format!(
                "JSON 反序列化失败 {}: {}",
                url, e
            ))
        })
    }

    /// 发送 GET 请求并反序列化 JSON，同时检测服务端 token 轮换。
    ///
    /// HOJ 服务端在 token 到期前会返回 `Refresh-Token: true` 头和新 `Authorization` 头。
    /// 返回 `(解析后的数据, 轮换后的新 token)`，未轮换时新 token 为 `None`。
    pub async fn get_json_with_refresh<T: DeserializeOwned>(
        &self,
        url: &str,
        auth_token: Option<&str>,
    ) -> AppResult<(T, Option<String>)> {
        let response = self.retry_get(url, auth_token).await?;
        let new_token = extract_refreshed_token(&response);
        let body = response.text().await.map_err(|e| {
            crate::core::error::AppError::Network(format!("读取响应体失败: {}", e))
        })?;
        let parsed = serde_json::from_str::<T>(&body).map_err(|e| {
            crate::core::error::AppError::Serialization(format!(
                "JSON 反序列化失败 {}: {}",
                url, e
            ))
        })?;
        Ok((parsed, new_token))
    }

    /// 发送 POST 请求（JSON body）并反序列化 JSON 响应体。
    ///
    /// POST 为非幂等方法，不执行自动重试。
    /// 对 4xx/5xx 错误直接返回 `AppError::Network`（含状态码）。
    pub async fn post_json<T: DeserializeOwned, B: Serialize>(
        &self,
        url: &str,
        body: &B,
        auth_token: Option<&str>,
    ) -> AppResult<T> {
        let mut req = self.client.post(url).json(body);
        if let Some(token) = auth_token {
            req = req.header("Authorization", token);
        }
        let response = req.send().await.map_err(|e| {
            crate::core::error::AppError::Network(format!("POST 请求失败 {}: {}", url, e))
        })?;
        if !response.status().is_success() {
            return Err(status_error(url, response.status()));
        }
        let body = response.text().await.map_err(|e| {
            crate::core::error::AppError::Network(format!("读取响应体失败: {}", e))
        })?;
        serde_json::from_str::<T>(&body).map_err(|e| {
            crate::core::error::AppError::Serialization(format!(
                "JSON 反序列化失败 {}: {}",
                url, e
            ))
        })
    }

    // ── 内部重试逻辑 ──

    /// 发送 GET 请求，对 5xx 响应自动重试（最多 2 次，指数退避）。
    /// 4xx 错误直接返回，不重试（非幂等场景客户端错误不应重试）。
    async fn retry_get(
        &self,
        url: &str,
        auth_token: Option<&str>,
    ) -> AppResult<reqwest::Response> {
        let mut last_error: Option<crate::core::error::AppError> = None;

        for attempt in 0..=MAX_RETRIES {
            let mut req = self.client.get(url);
            if let Some(token) = auth_token {
                req = req.header("Authorization", token);
            }

            match req.send().await {
                Ok(response) => {
                    // 4xx 客户端错误不重试，直接返回
                    if response.status().is_client_error() {
                        return Err(status_error(url, response.status()));
                    }
                    if response.status().is_server_error() && attempt < MAX_RETRIES {
                        let delay = retry_delay(attempt);
                        tokio::time::sleep(delay).await;
                        last_error = Some(crate::core::error::AppError::Network(format!(
                            "服务端错误 {} (status: {})，第 {} 次重试",
                            url,
                            response.status(),
                            attempt + 1
                        )));
                        continue;
                    }
                    return Ok(response);
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

        Err(last_error.unwrap_or_else(|| {
            crate::core::error::AppError::Network(format!("GET 请求失败（已达最大重试）: {}", url))
        }))
    }
}
