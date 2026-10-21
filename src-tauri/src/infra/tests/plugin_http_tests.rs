use super::*;

#[test]
fn ip_policy_rejects_special_targets_even_with_broad_private_grants() {
    let mut approval = NetworkApproval {
        origin: "https://example.com".into(),
        allow_http: false,
        private_cidrs: vec![],
    };
    assert!(allowed_address("8.8.8.8".parse().unwrap(), &approval));
    assert!(allowed_address(
        "2606:4700:4700::1111".parse().unwrap(),
        &approval
    ));
    assert!(!allowed_address("10.1.2.3".parse().unwrap(), &approval));
    approval.private_cidrs = vec!["0.0.0.0/0".into(), "::/0".into()];
    assert!(allowed_address("10.1.2.3".parse().unwrap(), &approval));
    for ip in [
        "127.0.0.1",
        "169.254.169.254",
        "0.0.0.0",
        "224.0.0.1",
        "100.64.0.1",
        "192.0.2.1",
        "::1",
        "::ffff:127.0.0.1",
        "fe80::1",
        "ff02::1",
        "2001:db8::1",
        "2002:7f00:1::",
    ] {
        assert!(!allowed_address(ip.parse().unwrap(), &approval), "{ip}");
    }
}
#[tokio::test]
async fn unapproved_origin_and_plaintext_fail_before_network() {
    let api = PluginHttp::new(
        crate::plugin::tests::support::authority("test.http"),
        vec![],
    )
    .unwrap();
    let c = api.authority.current_context().await.unwrap();
    for url in [
        "http://example.com",
        "https://example.com",
        "https://127.0.0.1",
    ] {
        let request = PluginHttpRequest {
            method: PluginHttpMethod::Get,
            url: url.into(),
            headers: Default::default(),
            body: None,
            timeout_ms: None,
        };
        assert!(api.request(&c, request).await.is_err());
    }
}

// Exercises the actual reqwest transport against a local fixture. Production policy
// still rejects loopback: only this private transport test bypasses DNS policy.
#[tokio::test]
async fn pinned_transport_does_not_redirect_and_bounds_response_bytes() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    for (reply, expected) in [
        ("HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/secret\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string(),Ok(302)),
        (format!("HTTP/1.1 200 OK\r\nContent-Length: 65537\r\nConnection: close\r\n\r\n{}","a".repeat(65537)),Err(PluginError::LimitExceeded)),
        ("HTTP/1.1 500 Error\r\nContent-Length: 5\r\nSet-Cookie: secret=1\r\nConnection: close\r\n\r\nerror".to_string(),Ok(500)),
    ] {
        let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let address=listener.local_addr().unwrap();
        let server=tokio::spawn(async move {
            let (mut socket,_)=listener.accept().await.unwrap();let mut bytes=vec![0;8192];let n=socket.read(&mut bytes).await.unwrap();
            let headers=String::from_utf8_lossy(&bytes[..n]).to_ascii_lowercase();
            assert!(!headers.contains("cookie:"));assert!(!headers.contains("authorization:"));
            let _=socket.write_all(reply.as_bytes()).await;
        });
        let api=PluginHttp::new(crate::plugin::tests::support::authority("test.http"),vec![]).unwrap();let c=api.authority.current_context().await.unwrap();
        let url=url::Url::parse(&format!("http://fixture.example:{}",address.port())).unwrap();
        let request=PluginHttpRequest { method:PluginHttpMethod::Get,url:url.to_string(),headers:Default::default(),body:None,timeout_ms:Some(5000) };
        let result=api.send_pinned(&c,request,url,vec![address]).await.map(|r| { assert!(!r.headers.contains_key("set-cookie"));r.status });
        assert_eq!(result,expected);server.await.unwrap();
    }
}

#[tokio::test(start_paused = true)]
async fn total_deadline_and_scope_cancellation_bound_stalled_http_reads() {
    use tokio::io::AsyncReadExt;
    for cancel in [false, true] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = [0; 8192];
            let _ = socket.read(&mut bytes).await;
            std::future::pending::<()>().await;
        });
        let api = PluginHttp::new(
            crate::plugin::tests::support::authority("test.http"),
            vec![],
        )
        .unwrap();
        let context = api.authority.current_context().await.unwrap();
        let url = url::Url::parse(&format!("http://fixture.example:{}", address.port())).unwrap();
        let request = PluginHttpRequest {
            method: PluginHttpMethod::Get,
            url: url.to_string(),
            headers: Default::default(),
            body: None,
            timeout_ms: Some(5000),
        };
        let scope = tokio_util::sync::CancellationToken::new();
        let revoke = scope.clone();
        let operation = api.bounded(
            scope,
            1000,
            api.send_pinned(&context, request, url, vec![address]),
        );
        let (result, ()) = tokio::join!(operation, async {
            if cancel {
                tokio::time::sleep(Duration::from_millis(100)).await;
                revoke.cancel();
            }
        });
        assert_eq!(
            result,
            Err(if cancel {
                PluginError::ContextExpired
            } else {
                PluginError::Timeout
            })
        );
        server.abort();
    }
}

#[tokio::test]
async fn public_request_runs_approval_ip_policy_budgets_and_projects_a_successful_response() {
    use crate::plugin::{permission::PluginGrant, query::ScopedQueries, tests::support::*};
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct FixtureTransport(Arc<AtomicUsize>);
    struct FixtureResolver;
    #[async_trait::async_trait]
    impl Resolver for FixtureResolver {
        async fn resolve(&self, host: &str, port: u16) -> std::io::Result<Vec<SocketAddr>> {
            assert_eq!(host, "fixture.example");
            assert_eq!(port, 80);
            Ok(vec!["8.8.8.8:80".parse().unwrap()])
        }
    }
    #[async_trait::async_trait]
    impl HttpTransport for FixtureTransport {
        async fn send(
            &self,
            authority: &ScopedQueries,
            context: &PluginQueryContext,
            request: PluginHttpRequest,
            url: url::Url,
            addresses: Vec<SocketAddr>,
        ) -> PluginResult<PluginHttpResponse> {
            authority.check(context).await?;
            assert_eq!(addresses, vec!["8.8.8.8:80".parse().unwrap()]);
            assert_eq!(url.origin().ascii_serialization(), "http://fixture.example");
            assert_eq!(request.body, Some(serde_json::json!({"value": 1})));
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(PluginHttpResponse {
                ok: true,
                status: 200,
                headers: BTreeMap::new(),
                body: "fixture response".into(),
            })
        }
    }
    let mut m = manifest("test.public-http");
    m.network.allow_origins = vec!["http://fixture.example".into()];
    let authority = Arc::new(ScopedQueries::new(
        PluginGrant::authorize(&m, &m.permissions).unwrap(),
        Arc::new(Source::default()),
        Arc::default(),
        Default::default(),
    ));
    let calls = Arc::new(AtomicUsize::new(0));
    let mut api = PluginHttp::new(
        authority.clone(),
        vec![NetworkApproval {
            origin: "http://fixture.example".into(),
            allow_http: true,
            private_cidrs: vec![],
        }],
    )
    .unwrap();
    api.transport = Arc::new(FixtureTransport(calls.clone()));
    api.resolver = Arc::new(FixtureResolver);
    let context = authority.current_context().await.unwrap();
    let request = PluginHttpRequest {
        method: PluginHttpMethod::Post,
        url: "http://fixture.example/path".into(),
        headers: BTreeMap::new(),
        body: Some(serde_json::json!({"value": 1})),
        timeout_ms: None,
    };
    let result = api.request(&context, request.clone()).await.unwrap();
    assert_eq!(result.status, 200);
    assert_eq!(result.body, "fixture response");
    let mut blocked = request;
    blocked.url = "http://127.0.0.1/path".into();
    assert!(api.request(&context, blocked).await.is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    authority.stopped.cancel();
}

#[tokio::test]
async fn identical_pinned_targets_reuse_a_client() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        for _ in 0..2 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = [0; 4096];
            assert!(socket.read(&mut bytes).await.unwrap() > 0);
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .await
                .unwrap();
        }
    });
    let transport = PinnedTransport {
        clients: Mutex::default(),
    };
    let authority = crate::plugin::tests::support::authority("test.pool");
    let context = authority.current_context().await.unwrap();
    let url = url::Url::parse(&format!("http://fixture.example:{}", address.port())).unwrap();
    for _ in 0..2 {
        transport
            .send(
                &authority,
                &context,
                PluginHttpRequest {
                    method: PluginHttpMethod::Get,
                    url: url.to_string(),
                    headers: BTreeMap::new(),
                    body: None,
                    timeout_ms: None,
                },
                url.clone(),
                vec![address],
            )
            .await
            .unwrap();
    }
    assert_eq!(transport.clients.lock().unwrap().len(), 1);
    server.await.unwrap();
}

struct BudgetResolver {
    stall: std::sync::atomic::AtomicBool,
    started: tokio::sync::Notify,
}
#[async_trait::async_trait]
impl Resolver for BudgetResolver {
    async fn resolve(&self, _: &str, port: u16) -> std::io::Result<Vec<SocketAddr>> {
        if self.stall.swap(false, std::sync::atomic::Ordering::SeqCst) {
            self.started.notify_one();
            std::future::pending::<()>().await;
        }
        Ok(vec![SocketAddr::new("8.8.8.8".parse().unwrap(), port)])
    }
}
struct BudgetTransport;
#[async_trait::async_trait]
impl HttpTransport for BudgetTransport {
    async fn send(
        &self,
        authority: &ScopedQueries,
        context: &PluginQueryContext,
        _: PluginHttpRequest,
        _: url::Url,
        _: Vec<SocketAddr>,
    ) -> PluginResult<PluginHttpResponse> {
        authority.check(context).await?;
        Ok(PluginHttpResponse {
            ok: true,
            status: 200,
            headers: BTreeMap::new(),
            body: String::new(),
        })
    }
}
fn budget_fixture(
    stall: bool,
) -> (
    Arc<PluginHttp>,
    Arc<crate::core::access::AccessRevision>,
    Arc<BudgetResolver>,
) {
    use crate::plugin::{permission::PluginGrant, tests::support::*};
    let access = Arc::new(crate::core::access::AccessRevision::default());
    let m = manifest("test.http-budget");
    let authority = Arc::new(ScopedQueries::new(
        PluginGrant::authorize(&m, &m.permissions).unwrap(),
        Arc::new(Source::default()),
        access.clone(),
        Default::default(),
    ));
    let mut api = PluginHttp::new(
        authority,
        vec![NetworkApproval {
            origin: "https://example.com".into(),
            allow_http: false,
            private_cidrs: vec![],
        }],
    )
    .unwrap();
    // An hour-long refill makes assertions independent of wall-clock scheduling.
    api.limiter = RateLimiter::direct(
        Quota::with_period(Duration::from_secs(3600))
            .unwrap()
            .allow_burst(NonZeroU32::new(2).unwrap()),
    );
    let resolver = Arc::new(BudgetResolver {
        stall: stall.into(),
        started: tokio::sync::Notify::new(),
    });
    api.resolver = resolver.clone();
    api.transport = Arc::new(BudgetTransport);
    (Arc::new(api), access, resolver)
}
fn budget_request() -> PluginHttpRequest {
    PluginHttpRequest {
        method: PluginHttpMethod::Get,
        url: "https://example.com".into(),
        headers: BTreeMap::new(),
        body: None,
        timeout_ms: None,
    }
}

#[tokio::test]
async fn concurrency_rejections_do_not_consume_http_rate_tokens() {
    let (api, _, _) = budget_fixture(false);
    let context = api.authority.current_context().await.unwrap();
    let permits = api.concurrent.try_acquire_many(2).unwrap();
    for _ in 0..5 {
        assert_eq!(
            api.request(&context, budget_request()).await,
            Err(PluginError::LimitExceeded)
        );
    }
    drop(permits);
    for _ in 0..2 {
        assert_eq!(
            api.request(&context, budget_request())
                .await
                .unwrap()
                .status,
            200
        );
    }
    assert_eq!(
        api.request(&context, budget_request()).await,
        Err(PluginError::LimitExceeded)
    );
    assert_eq!(api.concurrent.available_permits(), 2);
}

#[tokio::test]
async fn dropping_or_revoking_an_http_request_releases_its_concurrency_permit() {
    for abort in [true, false] {
        let (api, access, resolver) = budget_fixture(true);
        let context = api.authority.current_context().await.unwrap();
        let operation = {
            let api = api.clone();
            tokio::spawn(async move { api.request(&context, budget_request()).await })
        };
        tokio::time::timeout(Duration::from_secs(2), resolver.started.notified())
            .await
            .unwrap();
        assert_eq!(api.concurrent.available_permits(), 1);
        if abort {
            operation.abort();
            assert!(operation.await.unwrap_err().is_cancelled());
        } else {
            drop(access.begin());
            assert_eq!(operation.await.unwrap(), Err(PluginError::ContextExpired));
        }
        assert_eq!(api.concurrent.available_permits(), 2);
        let fresh = api.authority.current_context().await.unwrap();
        assert_eq!(
            api.request(&fresh, budget_request()).await.unwrap().status,
            200
        );
        assert_eq!(api.concurrent.available_permits(), 2);
    }
}
