use super::*;
use std::io::Write;
use std::sync::Mutex;

#[derive(Clone)]
struct Capture(Arc<Mutex<Vec<u8>>>);
impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn bridge_response_bounds_include_envelope_and_json_escaping() {
    let (response, success) = encode_response(Ok(json!("small")));
    assert!(success);
    assert_eq!(
        serde_json::from_str::<Value>(&response).unwrap()["value"],
        "small"
    );
    for value in [
        json!("a".repeat(MAX_BRIDGE_RESPONSE_BYTES)),
        json!("\n".repeat(MAX_BRIDGE_RESPONSE_BYTES / 2)),
    ] {
        let (response, success) = encode_response(Ok(value));
        assert!(!success);
        assert!(response.len() < MAX_BRIDGE_RESPONSE_BYTES);
        assert_eq!(
            serde_json::from_str::<Value>(&response).unwrap()["error"]["code"],
            "limit_exceeded"
        );
    }
    assert_eq!(
        encode("x".repeat(MAX_BRIDGE_RESPONSE_BYTES)),
        Err(PluginError::LimitExceeded)
    );
}

#[tokio::test]
async fn bridge_cas_audit_names_both_permissions_and_escapes_external_operations() {
    use tracing::instrument::WithSubscriber;
    struct Output;
    impl NotificationOutput for Output {
        fn emit(&self, _: &str, _: PluginNotificationLevel, _: &str) -> PluginResult<()> {
            Ok(())
        }
    }
    let dir = crate::test_support::TempDir::unique("bridge-audit");
    let queries = crate::plugin::tests::support::authority("test.audit");
    let api = ExecutionApi::new(
        queries.clone(),
        JsonPluginStorage::new(dir.to_path_buf(), queries.clone()),
        PluginHttp::new(queries, vec![]).unwrap(),
        Arc::new(Output),
    );
    let _sink = tracing::Dispatch::new(
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_writer(std::io::sink)
            .finish(),
    );
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let writer = Capture(bytes.clone());
    let dispatch = tracing::Dispatch::new(
        tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_max_level(tracing::Level::DEBUG)
            .with_writer(move || writer.clone())
            .finish(),
    );
    let response = call_json(
        &api,
        "storage.compareAndSet".into(),
        json!({
            "key": "fixture", "expected": {"state": "missing"}, "value": "private-value",
        })
        .to_string(),
    )
    .with_subscriber(dispatch.clone())
    .await;
    assert_eq!(
        serde_json::from_str::<Value>(&response).unwrap(),
        json!({"ok": true, "value": true})
    );
    call_json(&api, "unknown\nFORGED".into(), "{}".into())
        .with_subscriber(dispatch)
        .await;
    let log = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
    assert!(log.contains("storage.read storage.write"));
    assert!(log.contains("unknown\\nFORGED"));
    assert!(!log.contains("unknown\nFORGED"));
    assert!(!log.contains("private-value"));
}
