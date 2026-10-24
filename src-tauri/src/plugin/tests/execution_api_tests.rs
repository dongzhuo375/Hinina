use super::*;
use crate::infra::{plugin_http::PluginHttp, plugin_storage::JsonPluginStorage};
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

#[test]
fn bridge_admission_bounds_local_calls_per_activation() {
    struct Output;
    impl NotificationOutput for Output {
        fn emit(&self, _: &str, _: PluginNotificationLevel, _: &str) -> PluginResult<()> {
            Ok(())
        }
    }
    let dir = crate::test_support::TempDir::unique("bridge-admission");
    let queries = crate::plugin::test_support::authority("test.admission");
    let api = ExecutionApi::new(
        queries.clone(),
        Arc::new(JsonPluginStorage::new(dir.to_path_buf(), queries.clone())),
        Arc::new(PluginHttp::new(queries, vec![]).unwrap()),
        Arc::new(Output),
    );
    let mut admitted = 0;
    while api.admit().is_ok() {
        admitted += 1;
        assert!(admitted < 1_000, "admission gate must be bounded");
    }
    assert!(admitted >= 1);
    assert_eq!(api.admit(), Err(PluginError::LimitExceeded));
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
    let queries = crate::plugin::test_support::authority("test.audit");
    let api = ExecutionApi::new(
        queries.clone(),
        Arc::new(JsonPluginStorage::new(dir.to_path_buf(), queries.clone())),
        Arc::new(PluginHttp::new(queries, vec![]).unwrap()),
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

#[tokio::test]
async fn log_write_needs_no_permission_and_rate_limits_at_30_per_minute() {
    use tracing::instrument::WithSubscriber;
    struct Output;
    impl NotificationOutput for Output {
        fn emit(&self, _: &str, _: PluginNotificationLevel, _: &str) -> PluginResult<()> {
            Ok(())
        }
    }
    let dir = crate::test_support::TempDir::unique("log-rate");
    let manifest = crate::plugin::test_support::manifest("test.log");
    // Empty approval: effective permissions are the intersection, so this plugin has
    // none — and log.write must still work because the log outlet needs no permission.
    let queries = Arc::new(ScopedQueries::new(
        crate::plugin::permission::PluginGrant::authorize(&manifest, &[]).unwrap(),
        Arc::new(crate::plugin::test_support::Source::default()),
        Arc::default(),
        Default::default(),
    ));
    let api = ExecutionApi::new(
        queries.clone(),
        Arc::new(JsonPluginStorage::new(dir.to_path_buf(), queries.clone())),
        Arc::new(PluginHttp::new(queries, vec![]).unwrap()),
        Arc::new(Output),
    );
    // A live DEBUG-hint dispatch keeps the global level hint above INFO so these
    // lines pass the write gate and are actually charged against the budget.
    let sink = tracing::Dispatch::new(
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_writer(std::io::sink)
            .finish(),
    );
    // governor replenishes one token every 2 seconds, so a fixed "31st call fails"
    // assertion would flake under slow parallel execution: drain until the limit
    // with bounds instead.
    let mut written = 0u32;
    loop {
        let response = call_json(
            &api,
            "log.write".into(),
            json!({"level":"info","message":format!("line {written}")}).to_string(),
        )
        .with_subscriber(sink.clone())
        .await;
        let value = serde_json::from_str::<Value>(&response).unwrap();
        if value["ok"] == json!(false) {
            assert_eq!(value["error"]["code"], "limit_exceeded");
            // The rate-limit error object carries only the stable code, never log content.
            assert_eq!(value["error"].as_object().unwrap().len(), 1);
            break;
        }
        written += 1;
        assert!(written < 40, "log budget must stay bounded");
    }
    assert!(
        written >= 30,
        "log budget must allow the full 30/min quota (got {written})"
    );
}

#[tokio::test]
async fn log_write_skips_budget_for_level_filtered_lines() {
    use tracing::instrument::WithSubscriber;
    struct Output;
    impl NotificationOutput for Output {
        fn emit(&self, _: &str, _: PluginNotificationLevel, _: &str) -> PluginResult<()> {
            Ok(())
        }
    }
    let dir = crate::test_support::TempDir::unique("log-filtered");
    let queries = crate::plugin::test_support::authority("test.log-filtered");
    let api = ExecutionApi::new(
        queries.clone(),
        Arc::new(JsonPluginStorage::new(dir.to_path_buf(), queries.clone())),
        Arc::new(PluginHttp::new(queries, vec![]).unwrap()),
        Arc::new(Output),
    );
    // A host whose level filter excludes debug: those calls resolve Ok without
    // consuming the 30/min budget, so a later warn line is still admitted.
    let dispatch = tracing::Dispatch::new(
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_writer(std::io::sink)
            .finish(),
    );
    for i in 0..35 {
        let response = call_json(
            &api,
            "log.write".into(),
            json!({"level":"debug","message":format!("filtered {i}")}).to_string(),
        )
        .with_subscriber(dispatch.clone())
        .await;
        assert_eq!(
            serde_json::from_str::<Value>(&response).unwrap(),
            json!({"ok": true, "value": null})
        );
    }
    let response = call_json(
        &api,
        "log.write".into(),
        json!({"level":"warn","message":"still admitted"}).to_string(),
    )
    .with_subscriber(dispatch)
    .await;
    assert_eq!(
        serde_json::from_str::<Value>(&response).unwrap(),
        json!({"ok": true, "value": null})
    );
}

#[tokio::test]
async fn log_write_emits_sanitized_host_log_line() {
    use tracing::instrument::WithSubscriber;
    struct Output;
    impl NotificationOutput for Output {
        fn emit(&self, _: &str, _: PluginNotificationLevel, _: &str) -> PluginResult<()> {
            Ok(())
        }
    }
    let dir = crate::test_support::TempDir::unique("log-line");
    let queries = crate::plugin::test_support::authority("test.log-line");
    let api = ExecutionApi::new(
        queries.clone(),
        Arc::new(JsonPluginStorage::new(dir.to_path_buf(), queries.clone())),
        Arc::new(PluginHttp::new(queries, vec![]).unwrap()),
        Arc::new(Output),
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
    let response = async {
        // A sibling test emitting these callsites without an active subscriber can
        // cache process-global interest as "never" (first registration evaluates
        // against the registering thread's dispatcher). Rebuild while our
        // subscriber is the thread default — the calls below run in this same
        // poll, so there is no window to re-poison the cache.
        tracing::callsite::rebuild_interest_cache();
        call_json(
            &api,
            "log.write".into(),
            json!({"level":"warn","message":"stage=dispatch\nquoted=\"x\"\ttab"}).to_string(),
        )
        .await
    }
    .with_subscriber(dispatch)
    .await;
    assert_eq!(
        serde_json::from_str::<Value>(&response).unwrap(),
        json!({"ok": true, "value": null})
    );
    let log = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
    // The plugin log line: fixed identity fields plus the flattened, Debug-escaped message.
    assert!(log.contains("operation=\"plugin.log\""));
    assert!(log.contains("level=\"warn\""));
    assert!(log.contains("WARN"));
    assert!(log.contains("stage=dispatch quoted=\\\"x\\\" tab"));
    assert!(!log.contains("stage=dispatch\nquoted"));
    // The bridge audit line maps log.write to no permission.
    assert!(log.contains("operation=\"log.write\""));
    assert!(log.contains("permission=\"none\""));
}

#[tokio::test]
async fn contest_rank_bridge_maps_permission_and_bounds_oversized_pages() {
    use tracing::instrument::WithSubscriber;
    struct Output;
    impl NotificationOutput for Output {
        fn emit(&self, _: &str, _: PluginNotificationLevel, _: &str) -> PluginResult<()> {
            Ok(())
        }
    }
    let dir = crate::test_support::TempDir::unique("rank-bridge");
    let source = Arc::new(crate::plugin::test_support::Source::default());
    // A page larger than MAX_RANK_ROWS (Hydro returns the whole board) must fail
    // as limit_exceeded instead of being truncated by the 1MiB bridge cap.
    *source.rank_page.lock().unwrap() = Some(crate::core::entity::rank::ContestRankPage {
        records: (0..201)
            .map(|i| {
                serde_json::from_value(serde_json::json!({
                    "rank": i + 1, "uid": format!("u{i}"), "username": format!("team-{i}")
                }))
                .unwrap()
            })
            .collect(),
        total: 201,
        size: 201,
        current: 1,
        pages: 1,
    });
    let m = crate::plugin::test_support::manifest("test.rank-bridge");
    let queries = Arc::new(ScopedQueries::new(
        crate::plugin::permission::PluginGrant::authorize(&m, &m.permissions).unwrap(),
        source,
        Arc::default(),
        Default::default(),
    ));
    let api = ExecutionApi::new(
        queries.clone(),
        Arc::new(JsonPluginStorage::new(dir.to_path_buf(), queries.clone())),
        Arc::new(PluginHttp::new(queries, vec![]).unwrap()),
        Arc::new(Output),
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
    let context = ContextApi::current_context(&api).await.unwrap();
    let response = call_json(
        &api,
        "contest.rank".into(),
        json!({"context": context, "query": {"page": 1, "limit": 50}}).to_string(),
    )
    .with_subscriber(dispatch)
    .await;
    let value = serde_json::from_str::<Value>(&response).unwrap();
    assert_eq!(value["error"]["code"], "limit_exceeded");
    let log = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
    // The audit line maps the full operation to its own permission.
    assert!(log.contains("operation=\"contest.rank\""));
    assert!(log.contains("permission=\"contest.rank\""));
}
