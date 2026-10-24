use super::*;

#[test]
fn engine_errors_preserve_resource_and_script_categories() {
    assert_eq!(
        engine_error(rquickjs::Error::Allocation),
        PluginError::LimitExceeded
    );
    assert_eq!(
        engine_error(rquickjs::Error::Exception),
        PluginError::ScriptError
    );
}

#[tokio::test]
async fn rejection_tracker_observes_unhandled_promises_and_removes_handled_ones() {
    let mut runtime = QuickJsRuntime::new(CancellationToken::new()).await.unwrap();
    runtime
        .context
        .with(|ctx| {
            ctx.eval::<(), _>("globalThis.pending = Promise.reject('private credential');")
                .unwrap();
        })
        .await;
    assert_eq!(runtime.rejections.lock().unwrap().pending.len(), 1);
    runtime
        .context
        .with(|ctx| {
            ctx.eval::<(), _>("pending.catch(() => {});").unwrap();
        })
        .await;
    assert!(runtime.rejections.lock().unwrap().pending.is_empty());
    runtime
        .context
        .with(|ctx| {
            ctx.eval::<(), _>("Promise.reject('another secret').catch(() => {});")
                .unwrap();
        })
        .await;
    assert!(runtime.rejections.lock().unwrap().pending.is_empty());
    runtime.shutdown().await;
}

#[tokio::test]
async fn rejection_tracking_has_a_fixed_memory_budget() {
    let mut runtime = QuickJsRuntime::new(CancellationToken::new()).await.unwrap();
    runtime
        .context
        .with(|ctx| {
            ctx.eval::<(), _>(
                "globalThis.promises = Array.from({length: 300}, () => Promise.reject('secret'));",
            )
            .unwrap();
        })
        .await;
    {
        let state = runtime.rejections.lock().unwrap();
        assert_eq!(state.pending.len(), 256);
        assert!(state.overflow);
    }
    runtime.report_rejections();
    assert!(runtime.rejections.lock().unwrap().pending.is_empty());
    runtime.shutdown().await;
}

#[tokio::test]
async fn unhandled_rejection_diagnostics_never_format_js_reasons() {
    use std::io::Write;
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
    let _sink = tracing::Dispatch::new(
        tracing_subscriber::fmt()
            .with_writer(std::io::sink)
            .finish(),
    );
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let writer = Capture(bytes.clone());
    let dispatch = tracing::Dispatch::new(
        tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish(),
    );
    let mut runtime = QuickJsRuntime::new(CancellationToken::new()).await.unwrap();
    runtime.context.with(|ctx| {
        ctx.eval::<(), _>("Promise.reject({secret: 'PRIVATE_CREDENTIAL', toString() { throw Error('DO_NOT_FORMAT'); }});").unwrap();
    }).await;
    tracing::dispatcher::with_default(&dispatch, || runtime.report_rejections());
    let log = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
    assert!(log.contains("promise.rejection"));
    assert!(log.contains("unhandled_rejection"));
    assert!(!log.contains("PRIVATE_CREDENTIAL"));
    assert!(!log.contains("DO_NOT_FORMAT"));
    runtime.shutdown().await;
}

fn execution_api(
    source: Arc<crate::plugin::test_support::Source>,
    output: Arc<dyn crate::plugin::execution_api::NotificationOutput>,
    dir: &crate::test_support::TempDir,
) -> (Arc<dyn PluginApi>, Arc<crate::plugin::query::ScopedQueries>) {
    use crate::plugin::{
        execution_api::ExecutionApi, permission::PluginGrant, query::ScopedQueries,
        test_support::manifest,
    };
    let m = manifest("test.runtime-timeout");
    let queries = Arc::new(ScopedQueries::new(
        PluginGrant::authorize(&m, &m.permissions).unwrap(),
        source,
        Arc::default(),
        Default::default(),
    ));
    let api = Arc::new(ExecutionApi::new(
        queries.clone(),
        Arc::new(crate::infra::plugin_storage::JsonPluginStorage::new(
            dir.to_path_buf(),
            queries.clone(),
        )),
        Arc::new(crate::infra::plugin_http::PluginHttp::new(queries.clone(), vec![]).unwrap()),
        output,
    ));
    (api, queries)
}

#[tokio::test(start_paused = true)]
async fn real_js_catches_scan_timeouts_in_activation_and_event_and_remains_usable() {
    use crate::plugin::{
        api::{event::PluginEvent, notification::PluginNotificationLevel},
        execution_api::NotificationOutput,
        test_support::{record, Source},
    };
    struct Output;
    impl NotificationOutput for Output {
        fn emit(&self, _: &str, _: PluginNotificationLevel, _: &str) -> PluginResult<()> {
            Ok(())
        }
    }
    let dir = crate::test_support::TempDir::unique("runtime-scan-timeout");
    let source = Arc::new(Source {
        history_delay: Some(Duration::from_secs(5)),
        ..Default::default()
    });
    *source.pages.lock().unwrap() = (0..10)
        .map(|_| crate::core::entity::submission::SubmissionHistoryPage {
            records: vec![record("other")],
            total: None,
            has_more: true,
        })
        .collect();
    let (api, queries) = execution_api(source.clone(), Arc::new(Output), &dir);
    let mut runtime = QuickJsRuntime::new(CancellationToken::new()).await.unwrap();
    let script = r#"export default async h => {
      globalThis.caught = 0;
      async function scan() {
        const c = await h.context.current();
        try { await h.submission.get(c, 'contest', 'missing'); throw Error('unexpected result'); }
        catch (e) { if (e.code !== 'timeout') throw e; globalThis.caught++; }
        if (!(await h.identity.current(c)).userKey) throw Error('context lost');
      }
      await scan();
      h.events.on('resync.required', scan);
    }"#;
    let advance_scan = || async {
        source.history_started.notified().await;
        tokio::time::advance(Duration::from_secs(5)).await;
        source.history_started.notified().await;
        tokio::time::advance(Duration::from_secs(3)).await;
    };
    let (loaded, ()) = tokio::join!(
        runtime.load(
            script,
            api,
            &serde_json::Value::Null,
            vec!["resync.required".into()]
        ),
        advance_scan()
    );
    loaded.unwrap();
    let envelope = PluginEventEnvelope {
        version: 2,
        sequence: 1,
        occurred_at: 1,
        event: PluginEvent::ResyncRequired { lost: 0 },
    };
    let (delivered, ()) = tokio::join!(runtime.deliver(&envelope), advance_scan());
    delivered.unwrap();
    assert_eq!(
        runtime
            .context
            .with(|ctx| ctx.eval::<u32, _>("caught"))
            .await
            .unwrap(),
        2
    );
    assert!(!queries.stopped.is_cancelled());
    let current = queries.current_context().await.unwrap();
    queries.contest(&current, None).await.unwrap();
    runtime.shutdown().await;
}

#[tokio::test]
async fn late_host_calls_return_catchable_timeout_before_hard_execution_deadline() {
    use crate::plugin::{
        api::{event::PluginEvent, notification::PluginNotificationLevel},
        execution_api::NotificationOutput,
        test_support::Source,
    };
    // Compress only the test's remaining window; exercise the production bridge
    // cutoff anchored to the actual runtime deadline, in both entry kinds.
    struct Arm(Arc<Mutex<Option<Instant>>>);
    impl NotificationOutput for Arm {
        fn emit(&self, _: &str, _: PluginNotificationLevel, _: &str) -> PluginResult<()> {
            *self.0.lock().unwrap() =
                Some(Instant::now() + API_ERROR_GRACE + Duration::from_millis(100));
            Ok(())
        }
    }
    let dir = crate::test_support::TempDir::unique("runtime-late-call");
    let mut runtime = QuickJsRuntime::new(CancellationToken::new()).await.unwrap();
    let (api, _) = execution_api(
        Arc::new(Source {
            pending: true,
            ..Default::default()
        }),
        Arc::new(Arm(runtime.deadline.clone())),
        &dir,
    );
    let script = r#"export default async h => {
      globalThis.caught = 0;
      async function lateCall() {
        const c = await h.context.current();
        await h.notification.info('arm');
        try { await h.contest.current(c); throw Error('unexpected result'); }
        catch (e) { if (e.code !== 'timeout') throw e; globalThis.caught++; }
      }
      await lateCall();
      h.events.on('resync.required', lateCall);
    }"#;
    runtime
        .load(
            script,
            api,
            &serde_json::Value::Null,
            vec!["resync.required".into()],
        )
        .await
        .unwrap();
    runtime
        .deliver(&PluginEventEnvelope {
            version: 2,
            sequence: 1,
            occurred_at: 1,
            event: PluginEvent::ResyncRequired { lost: 0 },
        })
        .await
        .unwrap();
    assert_eq!(
        runtime
            .context
            .with(|ctx| ctx.eval::<u32, _>("caught"))
            .await
            .unwrap(),
        2
    );
    runtime.shutdown().await;
}

#[tokio::test]
async fn a_rejected_handler_does_not_stop_later_handlers_or_events() {
    use crate::plugin::{
        api::event::PluginEvent, api::notification::PluginNotificationLevel,
        execution_api::NotificationOutput, test_support::Source,
    };
    struct Output;
    impl NotificationOutput for Output {
        fn emit(&self, _: &str, _: PluginNotificationLevel, _: &str) -> PluginResult<()> {
            Ok(())
        }
    }
    let dir = crate::test_support::TempDir::unique("runtime-handler-failure");
    let (api, _queries) = execution_api(Arc::new(Source::default()), Arc::new(Output), &dir);
    let mut runtime = QuickJsRuntime::new(CancellationToken::new()).await.unwrap();
    let script = r#"export default async h => {
      globalThis.calls = 0;
      h.events.on('resync.required', async () => { globalThis.calls += 1; throw Object.freeze({ code: 'limit_exceeded' }); });
      h.events.on('resync.required', async () => { globalThis.calls += 1; });
    }"#;
    runtime
        .load(
            script,
            api,
            &serde_json::Value::Null,
            vec!["resync.required".into()],
        )
        .await
        .unwrap();
    let envelope = PluginEventEnvelope {
        version: 2,
        sequence: 1,
        occurred_at: 1,
        event: PluginEvent::ResyncRequired { lost: 0 },
    };
    // A rejected handler is isolated: the remaining handler and later events still run,
    // and the per-delivery failure count is surfaced so the host can prompt a resync.
    let first = runtime.deliver(&envelope).await.unwrap();
    let second = runtime.deliver(&envelope).await.unwrap();
    assert_eq!(first.failed_handlers, 1);
    assert_eq!(second.failed_handlers, 1);
    assert_eq!(
        runtime
            .context
            .with(|ctx| ctx.eval::<u32, _>("calls"))
            .await
            .unwrap(),
        4
    );
    runtime.shutdown().await;
}

#[tokio::test]
async fn dropping_a_loaded_runtime_without_shutdown_releases_the_persistent_handler() {
    use crate::plugin::{
        api::notification::PluginNotificationLevel, execution_api::NotificationOutput,
        test_support::Source,
    };
    struct Output;
    impl NotificationOutput for Output {
        fn emit(&self, _: &str, _: PluginNotificationLevel, _: &str) -> PluginResult<()> {
            Ok(())
        }
    }
    let dir = crate::test_support::TempDir::unique("runtime-drop");
    let (api, _queries) = execution_api(Arc::new(Source::default()), Arc::new(Output), &dir);
    let mut runtime = QuickJsRuntime::new(CancellationToken::new()).await.unwrap();
    runtime
        .load(
            "export default async () => {};",
            api,
            &serde_json::Value::Null,
            vec![],
        )
        .await
        .unwrap();
    // A worker panic unwinds without `shutdown`; the drop guard must release the
    // persistent handler before the engine, otherwise rquickjs aborts the process.
    drop(runtime);
}

#[tokio::test]
async fn real_js_log_calls_resolve_undefined_without_throwing() {
    use crate::plugin::{
        api::notification::PluginNotificationLevel, execution_api::NotificationOutput,
        test_support::Source,
    };
    struct Output;
    impl NotificationOutput for Output {
        fn emit(&self, _: &str, _: PluginNotificationLevel, _: &str) -> PluginResult<()> {
            Ok(())
        }
    }
    let dir = crate::test_support::TempDir::unique("runtime-log");
    let (api, _queries) = execution_api(Arc::new(Source::default()), Arc::new(Output), &dir);
    let mut runtime = QuickJsRuntime::new(CancellationToken::new()).await.unwrap();
    let script = r#"export default async h => {
      globalThis.resolved = 0;
      for (const level of ['debug', 'info', 'warn', 'error']) {
        // Void host calls resolve with no value (bridge JSON null, like storage.set);
        // the contract under test is "settles, does not throw, carries no value".
        if ((await h.log[level]('hello ' + level)) == null) globalThis.resolved++;
      }
    }"#;
    runtime
        .load(script, api, &serde_json::Value::Null, vec![])
        .await
        .unwrap();
    assert_eq!(
        runtime
            .context
            .with(|ctx| ctx.eval::<u32, _>("resolved"))
            .await
            .unwrap(),
        4
    );
    runtime.shutdown().await;
}

#[tokio::test]
async fn real_js_rank_and_problems_return_full_json_with_plain_object_cells() {
    use crate::plugin::{
        api::notification::PluginNotificationLevel, execution_api::NotificationOutput,
        test_support::Source,
    };
    struct Output;
    impl NotificationOutput for Output {
        fn emit(&self, _: &str, _: PluginNotificationLevel, _: &str) -> PluginResult<()> {
            Ok(())
        }
    }
    let dir = crate::test_support::TempDir::unique("runtime-rank");
    let source = Arc::new(Source::default());
    *source.rank_page.lock().unwrap() = Some(
        serde_json::from_value(serde_json::json!({
            "records": [{
                "rank": 1, "uid": "user-1", "username": "team-display", "school": "School A",
                "ac": 3, "totalScore": 300,
                "submissionInfo": { "A": { "errorNum": 2, "isAc": true, "isFirstAc": true, "acTime": 120 } }
            }],
            "total": 1, "size": 50, "current": 1, "pages": 1
        }))
        .unwrap(),
    );
    *source.problems.lock().unwrap() = vec![
        serde_json::from_value(serde_json::json!({
            "id": 1, "displayId": "A", "cid": "contest", "problemId": "P1001",
            "displayTitle": "A + B", "color": "#FF0000"
        }))
        .unwrap(),
    ];
    let (api, _queries) = execution_api(source, Arc::new(Output), &dir);
    let mut runtime = QuickJsRuntime::new(CancellationToken::new()).await.unwrap();
    let script = r#"export default async h => {
      const c = await h.context.current();
      const page = await h.contest.rank(c, { page: 1, limit: 50 });
      globalThis.rows = page.rows.length;
      globalThis.cellOk = typeof page.rows[0].cells.A === 'object'
        && page.rows[0].cells.A.firstAc === true
        && page.rows[0].cells.A.acTime === 120 ? 1 : 0;
      globalThis.problems = (await h.contest.problems(c)).length;
    }"#;
    runtime
        .load(script, api, &serde_json::Value::Null, vec![])
        .await
        .unwrap();
    for expression in ["rows", "cellOk", "problems"] {
        assert_eq!(
            runtime
                .context
                .with(|ctx| ctx.eval::<u32, _>(expression))
                .await
                .unwrap(),
            1,
            "{expression}"
        );
    }
    runtime.shutdown().await;
}
