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
        let request=PluginHttpRequest { method:PluginHttpMethod::Get,url:url.to_string(),headers:Default::default(),body:None,timeout_ms:Some(100) };
        let result=api.send_pinned(&c,request,url,vec![address]).await.map(|r| { assert!(!r.headers.contains_key("set-cookie"));r.status });
        assert_eq!(result,expected);server.await.unwrap();
    }
}

#[tokio::test]
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
            timeout_ms: Some(20),
        };
        let scope = tokio_util::sync::CancellationToken::new();
        let revoke = scope.clone();
        let operation = api.bounded(
            scope,
            40,
            api.send_pinned(&context, request, url, vec![address]),
        );
        let (result, ()) = tokio::join!(operation, async {
            if cancel {
                tokio::time::sleep(Duration::from_millis(10)).await;
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
