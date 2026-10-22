//! @plugin-module
//! @layer infra/plugin_http
//! @version 2
//! @stability stable
//! Isolated HTTP: exact origins, all DNS answers checked then pinned, no cookies/proxy/redirects/retries.
use super::plugin_diagnostics as diagnostics;
use crate::plugin::{
    api::{http::*, *},
    host::manifest::PluginPermission,
    query::ScopedQueries,
};
use governor::{DefaultDirectRateLimiter, Quota, RateLimiter};
use std::{
    collections::{BTreeMap, VecDeque},
    net::{IpAddr, SocketAddr},
    num::NonZeroU32,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::Semaphore;

#[async_trait::async_trait]
trait Resolver: Send + Sync {
    async fn resolve(&self, host: &str, port: u16) -> std::io::Result<Vec<SocketAddr>>;
}
struct SystemResolver;
#[async_trait::async_trait]
impl Resolver for SystemResolver {
    async fn resolve(&self, host: &str, port: u16) -> std::io::Result<Vec<SocketAddr>> {
        Ok(tokio::net::lookup_host((host, port))
            .await?
            .take(65)
            .collect())
    }
}

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
                && !reserved_v6()
                    .iter()
                    .any(|net| net.contains(&IpAddr::V6(ip)));
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

/// Parsed once: these ranges are checked for every resolved address of every request.
fn reserved_v6() -> &'static [ipnet::IpNet] {
    static RANGES: std::sync::OnceLock<[ipnet::IpNet; 3]> = std::sync::OnceLock::new();
    RANGES.get_or_init(|| {
        [
            "2001::/23".parse().expect("literal CIDR"),
            "2002::/16".parse().expect("literal CIDR"),
            "3fff::/20".parse().expect("literal CIDR"),
        ]
    })
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
    transport: Arc<dyn HttpTransport>,
    resolver: Arc<dyn Resolver>,
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
            approval.validate().inspect_err(|_| {
                diagnostics::failure(
                    Some(&authority.grant.manifest().id),
                    "http.approval",
                    "invalid_approval",
                )
            })?;
        }
        Ok(Self {
            authority,
            approvals,
            limiter: RateLimiter::direct(Quota::per_minute(NonZeroU32::new(60).unwrap())),
            concurrent: Semaphore::new(2),
            resolver: Arc::new(SystemResolver),
            transport: Arc::new(PinnedTransport {
                clients: Mutex::default(),
            }),
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
        let scope = self.authority.check(context).await?;
        let _permit = self
            .concurrent
            .try_acquire()
            .map_err(|_| PluginError::LimitExceeded)?;
        if self.limiter.check().is_err() {
            return Err(PluginError::LimitExceeded);
        }
        let timeout = request
            .timeout_ms
            .unwrap_or(5000)
            .min(self.authority.grant.manifest().network.timeout_ms);
        let operation = async move {
            let _permit = _permit;
            self.send(context, request).await
        };
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
            result = tokio::time::timeout(Duration::from_millis(timeout.into()), operation) => result.map_err(|_| {
                diagnostics::failure(Some(&self.authority.grant.manifest().id), "http.deadline", "timeout");
                PluginError::Timeout
            })?,
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
            self.resolver.resolve(host, port).await.map_err(|error| {
                diagnostics::io(
                    Some(&self.authority.grant.manifest().id),
                    "http.resolve",
                    &error,
                );
                PluginError::Unavailable
            })?
        };
        if addresses.is_empty()
            || addresses.len() > 64
            || addresses.iter().any(|a| !allowed_address(a.ip(), approval))
        {
            diagnostics::failure(
                Some(&self.authority.grant.manifest().id),
                "http.address_policy",
                "rejected_addresses",
            );
            return Err(PluginError::InvalidInput);
        }
        self.send_pinned(context, request, url, addresses).await
    }
    // Private helper for policy-independent transport fixtures.
    async fn send_pinned(
        &self,
        context: &PluginQueryContext,
        request: PluginHttpRequest,
        url: url::Url,
        addresses: Vec<SocketAddr>,
    ) -> PluginResult<PluginHttpResponse> {
        self.transport
            .send(self.authority.as_ref(), context, request, url, addresses)
            .await
    }
}

#[async_trait::async_trait]
impl PluginHttpApi for PluginHttp {
    async fn request(
        &self,
        context: &PluginQueryContext,
        request: PluginHttpRequest,
    ) -> PluginResult<PluginHttpResponse> {
        PluginHttp::request(self, context, request).await
    }
}

#[async_trait::async_trait]
trait HttpTransport: Send + Sync {
    async fn send(
        &self,
        authority: &ScopedQueries,
        context: &PluginQueryContext,
        request: PluginHttpRequest,
        url: url::Url,
        addresses: Vec<SocketAddr>,
    ) -> PluginResult<PluginHttpResponse>;
}
struct PinnedTransport {
    clients: Mutex<VecDeque<(String, Vec<SocketAddr>, reqwest::Client)>>,
}
#[async_trait::async_trait]
impl HttpTransport for PinnedTransport {
    // The production caller has checked every address before reaching this transport.
    async fn send(
        &self,
        authority: &ScopedQueries,
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
        // Reuse only for an identical origin and freshly validated DNS address set.
        // Never let a pooled connection bypass a changed DNS/IP approval decision.
        let origin = url.origin().ascii_serialization();
        let cached = self
            .clients
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .find(|(key, pinned, _)| key == &origin && pinned == &addresses)
            .map(|(_, _, client)| client.clone());
        let client = match cached {
            Some(client) => client,
            None => {
                let client = reqwest::Client::builder()
                    .no_proxy()
                    .redirect(reqwest::redirect::Policy::none())
                    .retry(reqwest::retry::never())
                    .resolve_to_addrs(host, &addresses)
                    .build()
                    .map_err(|error| {
                        diagnostics::http(
                            Some(&authority.grant.manifest().id),
                            "http.client",
                            &error,
                        );
                        PluginError::Unavailable
                    })?;
                let mut clients = self.clients.lock().unwrap_or_else(|e| e.into_inner());
                if clients.len() >= 32 {
                    clients.pop_front();
                }
                clients.push_back((origin, addresses.clone(), client.clone()));
                client
            }
        };
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
            let bytes = serde_json::to_vec(&body).map_err(|error| {
                diagnostics::json(Some(&authority.grant.manifest().id), "http.encode", &error);
                PluginError::InvalidInput
            })?;
            if bytes.len() > authority.grant.manifest().network.max_body_bytes as usize {
                return Err(PluginError::LimitExceeded);
            }
            builder = builder
                .header("content-type", "application/json")
                .body(bytes);
        }
        authority.check(context).await?;
        let mut response = builder.send().await.map_err(|e| {
            diagnostics::http(Some(&authority.grant.manifest().id), "http.send", &e);
            if e.is_timeout() {
                PluginError::Timeout
            } else {
                PluginError::Unavailable
            }
        })?;
        // Post-send peer check is defense in depth; pre-send DNS validation/pinning is
        // the prevention boundary. A sent request's remote side effects cannot be rolled back.
        if response
            .remote_addr()
            .is_none_or(|a| !addresses.contains(&a))
        {
            diagnostics::failure(
                Some(&authority.grant.manifest().id),
                "http.peer",
                "unexpected_address",
            );
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
        while let Some(chunk) = response.chunk().await.map_err(|error| {
            diagnostics::http(Some(&authority.grant.manifest().id), "http.read", &error);
            PluginError::Unavailable
        })? {
            if body.len() + chunk.len() > MAX_HTTP_BODY_BYTES {
                return Err(PluginError::LimitExceeded);
            }
            body.extend_from_slice(&chunk);
        }
        authority.check(context).await?;
        Ok(PluginHttpResponse {
            ok: (200..300).contains(&status),
            status,
            headers,
            body: String::from_utf8(body).map_err(|_| {
                diagnostics::failure(
                    Some(&authority.grant.manifest().id),
                    "http.decode",
                    "invalid_utf8",
                );
                PluginError::Unavailable
            })?,
        })
    }
}
#[cfg(test)]
#[path = "tests/plugin_http_tests.rs"]
mod tests;
