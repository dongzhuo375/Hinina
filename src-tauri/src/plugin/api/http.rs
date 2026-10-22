//! @plugin-module
//! @layer plugin/api
//! @version 2
//! @stability stable
//! 网络 DTO、形状检查与宿主审批数据。执行器位于 infra/plugin_http。
use super::{PluginError, PluginQueryContext, PluginResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// @plugin-api
/// @since 1.0
/// @permission network.request
/// @stability stable
/// 请求/响应体宿主硬上限；Manifest 可进一步收紧请求体。
pub const MAX_HTTP_BODY_BYTES: usize = 65_536;

/// @plugin-api
/// @since 1.0
/// @permission network.request
/// @stability stable
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PluginHttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

/// @plugin-api
/// @since 1.0
/// @permission network.request
/// @stability stable
/// JSON 请求体，禁止危险代理/客户端配置；不自动附加 Hinina 凭证。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginHttpRequest {
    pub method: PluginHttpMethod,
    pub url: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    pub body: Option<serde_json::Value>,
    pub timeout_ms: Option<u32>,
}

impl PluginHttpRequest {
    /// @plugin-api
    /// @since 1.0
    /// @permission network.request
    /// @stability stable
    /// 仅验证输入边界，不代表 URL 安全。上限为 URL 2048、headers 32/8192 字节、
    /// body 65536 字节、timeout 1..=5000 ms；错误 InvalidInput/LimitExceeded。
    pub fn validate_shape(&self) -> PluginResult<()> {
        if self.url.is_empty() || self.url.chars().any(char::is_control) {
            return Err(PluginError::InvalidInput);
        }
        if self.url.len() > 2048
            || self.headers.len() > 32
            || self
                .headers
                .iter()
                .map(|(k, v)| k.len() + v.len())
                .sum::<usize>()
                > 8192
            || self.timeout_ms.is_some_and(|n| !(1..=5000).contains(&n))
        {
            return Err(PluginError::LimitExceeded);
        }
        for (name, value) in &self.headers {
            let lower = name.to_ascii_lowercase();
            if name.is_empty()
                || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                || value.chars().any(char::is_control)
                || matches!(
                    lower.as_str(),
                    "host"
                        | "cookie"
                        | "connection"
                        | "upgrade"
                        | "transfer-encoding"
                        | "content-length"
                        | "content-type"
                )
                || lower.starts_with("proxy-")
            {
                return Err(PluginError::InvalidInput);
            }
        }
        if let Some(body) = &self.body {
            let bytes = serde_json::to_vec(body).map_err(|_| PluginError::InvalidInput)?;
            if bytes.len() > MAX_HTTP_BODY_BYTES {
                return Err(PluginError::LimitExceeded);
            }
        }
        Ok(())
    }
}

/// @plugin-api
/// @since 1.0
/// @permission network.request
/// @stability stable
/// 有限长 UTF-8 文本响应；仅公开 content-type/retry-after 响应头，不暴露 Set-Cookie。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PluginHttpResponse {
    pub ok: bool,
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: String,
}

/// @plugin-api
/// @since 1.0
/// @permission network.request
/// @stability stable
/// 实现必须验证上下文和授予权限、独立无 Cookie 客户端、精确 origin 与宿主批准的地址范围、
/// 全部 DNS/连接 IP 必须通过策略，禁重定向/代理，每插件 60 次/分钟；不自动重试。
#[async_trait]
pub trait PluginHttpApi: Send + Sync {
    /// @plugin-api
    /// @since 1.0
    /// @permission network.request
    /// @stability stable
    /// 非 2xx 作为响应返回；网络失败 Unavailable，超时 Timeout，超限 LimitExceeded。
    async fn request(
        &self,
        context: &PluginQueryContext,
        request: PluginHttpRequest,
    ) -> PluginResult<PluginHttpResponse>;
}

/// @plugin-api
/// @since 2.0
/// @permission host-only
/// @stability stable
/// An origin and optional private address ranges explicitly approved outside plugin code.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkApproval {
    pub origin: String,
    #[serde(default)]
    pub allow_http: bool,
    #[serde(default)]
    pub private_cidrs: Vec<String>,
}
impl NetworkApproval {
    /// @plugin-api
    /// @since 2.0
    /// @stability stable
    pub fn validate(&self) -> PluginResult<()> {
        if !crate::plugin::host::manifest::valid_origin(&self.origin)
            || (self.origin.starts_with("http:") && !self.allow_http)
            || self.private_cidrs.len() > 32
            || self
                .private_cidrs
                .iter()
                .any(|s| s.parse::<ipnet::IpNet>().is_err())
        {
            return Err(PluginError::InvalidInput);
        }
        Ok(())
    }
}
