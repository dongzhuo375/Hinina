//! @plugin-module
//! @layer infra/plugin_http
//! @version 2
//! @stability stable
//! Isolated HTTP: exact origins, all DNS answers checked then pinned, no cookies/proxy/redirects/retries.
use crate::plugin::{
    api::{http::*, *},
    host::manifest::PluginPermission,
    query::ScopedQueries,
};
use governor::{DefaultDirectRateLimiter, Quota, RateLimiter};
use std::{
    collections::BTreeMap,
    net::{IpAddr, SocketAddr},
    num::NonZeroU32,
    sync::Arc,
    time::Duration,
};
use tokio::sync::Semaphore;

/// @plugin-api
/// @since 2.0
/// @stability stable
/// Public unicast or explicitly approved RFC1918/ULA. Special-use targets stay forbidden.
pub fn allowed_address(ip: IpAddr, approval: &NetworkApproval) -> bool {
    let (public, private) = match ip {
        IpAddr::V4(ip) => {
            let network = ip_network::Ipv4Network::from(ip);
            (network.is_global() && !ip.is_multicast(), ip.is_private())
        }
        IpAddr::V6(ip) => {
            // Restrict to direct global unicast; exclude transition, special-use and documentation ranges.
            let network = ip_network::Ipv6Network::from(ip);
            let global = ip.segments()[0] & 0xe000 == 0x2000
                && network.is_global()
                && !network.is_documentation()
                && !"2001::/23"
                    .parse::<ipnet::IpNet>()
                    .unwrap()
                    .contains(&IpAddr::V6(ip))
                && !"2002::/16"
                    .parse::<ipnet::IpNet>()
                    .unwrap()
                    .contains(&IpAddr::V6(ip))
                && !"3fff::/20"
                    .parse::<ipnet::IpNet>()
                    .unwrap()
                    .contains(&IpAddr::V6(ip));
            (global, ip.is_unique_local())
        }
    };
    public
        || (private
            && approval
                .private_cidrs
                .iter()
                .filter_map(|s| s.parse::<ipnet::IpNet>().ok())
                .any(|net| net.contains(&ip)))
}

/// @plugin-api
/// @since 2.0
/// @stability stable
/// Per-plugin budgets. No access to Hinina's shared OJ HTTP client.
pub struct PluginHttp {
    authority: Arc<ScopedQueries>,
    approvals: Vec<NetworkApproval>,
    limiter: DefaultDirectRateLimiter,
    concurrent: Semaphore,
}
impl PluginHttp {
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    pub fn new(
        authority: Arc<ScopedQueries>,
        approvals: Vec<NetworkApproval>,
    ) -> PluginResult<Self> {
        for approval in &approvals {
            approval.validate()?;
        }
        Ok(Self {
            authority,
            approvals,
            limiter: RateLimiter::direct(Quota::per_minute(NonZeroU32::new(60).unwrap())),
            concurrent: Semaphore::new(2),
        })
    }
    /// @plugin-api
    /// @since 2.0
    /// @permission network.request
    /// @stability stable
    /// Cancellation cannot roll back a request already accepted by an external service.
    pub async fn request(
        &self,
        context: &PluginQueryContext,
        request: PluginHttpRequest,
    ) -> PluginResult<PluginHttpResponse> {
        self.authority.require(PluginPermission::NetworkRequest)?;
        request.validate_shape()?;
        let scope = self.authority.check(context)?;
        if self.limiter.check().is_err() {
            return Err(PluginError::LimitExceeded);
        }
        let _permit = self
            .concurrent
            .try_acquire()
            .map_err(|_| PluginError::LimitExceeded)?;
        let timeout = request
            .timeout_ms
            .unwrap_or(5000)
            .min(self.authority.grant.manifest().network.timeout_ms);
        let operation = self.send(context, request);
        self.bounded(scope, timeout, operation).await
    }
    async fn bounded(
        &self,
        scope: tokio_util::sync::CancellationToken,
        timeout: u32,
        operation: impl std::future::Future<Output = PluginResult<PluginHttpResponse>>,
    ) -> PluginResult<PluginHttpResponse> {
        tokio::select! { biased;
            _ = self.authority.stopped.cancelled() => Err(PluginError::ContextExpired),
            _ = scope.cancelled() => Err(PluginError::ContextExpired),
            result = tokio::time::timeout(Duration::from_millis(timeout.into()), operation) => result.map_err(|_| PluginError::Timeout)?,
        }
    }
    async fn send(
        &self,
        context: &PluginQueryContext,
        request: PluginHttpRequest,
    ) -> PluginResult<PluginHttpResponse> {
        let url = url::Url::parse(&request.url).map_err(|_| PluginError::InvalidInput)?;
        if !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
            return Err(PluginError::InvalidInput);
        }
        let origin = url.origin().ascii_serialization();
        let approval = self
            .approvals
            .iter()
            .find(|a| a.origin == origin)
            .filter(|_| {
                self.authority
                    .grant
                    .manifest()
                    .network
                    .allow_origins
                    .contains(&origin)
            })
            .ok_or(PluginError::PermissionDenied {
                permission: PluginPermission::NetworkRequest,
            })?;
        let host = url
            .host_str()
            .ok_or(PluginError::InvalidInput)?
            .trim_start_matches('[')
            .trim_end_matches(']');
        let port = url
            .port_or_known_default()
            .ok_or(PluginError::InvalidInput)?;
        let addresses: Vec<SocketAddr> = if let Ok(ip) = host.parse::<IpAddr>() {
            vec![SocketAddr::new(ip, port)]
        } else {
            tokio::net::lookup_host((host, port))
                .await
                .map_err(|_| PluginError::Unavailable)?
                .take(65)
                .collect()
        };
        if addresses.is_empty()
            || addresses.len() > 64
            || addresses.iter().any(|a| !allowed_address(a.ip(), approval))
        {
            return Err(PluginError::InvalidInput);
        }
        self.send_pinned(context, request, url, addresses).await
    }
    // The production caller has checked every address before reaching this transport.
    async fn send_pinned(
        &self,
        context: &PluginQueryContext,
        request: PluginHttpRequest,
        url: url::Url,
        addresses: Vec<SocketAddr>,
    ) -> PluginResult<PluginHttpResponse> {
        let host = url
            .host_str()
            .ok_or(PluginError::InvalidInput)?
            .trim_start_matches('[')
            .trim_end_matches(']');
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .resolve_to_addrs(host, &addresses)
            .build()
            .map_err(|_| PluginError::Unavailable)?;
        let method = match request.method {
            PluginHttpMethod::Get => reqwest::Method::GET,
            PluginHttpMethod::Post => reqwest::Method::POST,
            PluginHttpMethod::Put => reqwest::Method::PUT,
            PluginHttpMethod::Patch => reqwest::Method::PATCH,
            PluginHttpMethod::Delete => reqwest::Method::DELETE,
        };
        let mut builder = client.request(method, url.clone());
        for (key, value) in request.headers {
            builder = builder.header(key, value);
        }
        if let Some(body) = request.body {
            let bytes = serde_json::to_vec(&body).map_err(|_| PluginError::InvalidInput)?;
            if bytes.len() > self.authority.grant.manifest().network.max_body_bytes as usize {
                return Err(PluginError::LimitExceeded);
            }
            builder = builder
                .header("content-type", "application/json")
                .body(bytes);
        }
        self.authority.check(context)?;
        let mut response = builder.send().await.map_err(|e| {
            if e.is_timeout() {
                PluginError::Timeout
            } else {
                PluginError::Unavailable
            }
        })?;
        if response
            .remote_addr()
            .is_none_or(|a| !addresses.contains(&a))
        {
            return Err(PluginError::Unavailable);
        }
        let status = response.status().as_u16();
        let headers = ["content-type", "retry-after"]
            .into_iter()
            .filter_map(|name| {
                response
                    .headers()
                    .get(name)
                    .and_then(|v| v.to_str().ok())
                    .filter(|s| s.len() <= 1024)
                    .map(|value| (name.to_string(), value.to_string()))
            })
            .collect::<BTreeMap<_, _>>();
        if response
            .content_length()
            .is_some_and(|n| n > MAX_HTTP_BODY_BYTES as u64)
        {
            return Err(PluginError::LimitExceeded);
        }
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| PluginError::Unavailable)?
        {
            if body.len() + chunk.len() > MAX_HTTP_BODY_BYTES {
                return Err(PluginError::LimitExceeded);
            }
            body.extend_from_slice(&chunk);
        }
        self.authority.check(context)?;
        Ok(PluginHttpResponse {
            ok: (200..300).contains(&status),
            status,
            headers,
            body: String::from_utf8(body).map_err(|_| PluginError::Unavailable)?,
        })
    }
}
#[cfg(test)]
#[path = "tests/plugin_http_tests.rs"]
mod tests;
