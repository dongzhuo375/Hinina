use crate::plugin::test_support::*;
use crate::{
    core::{
        access::AccessRevision,
        event::{core_event::CoreEvent, core_event_bus::CoreEventBus},
    },
    plugin::{
        api::{notification::PluginNotificationLevel, *},
        execution_api::NotificationOutput,
        host::manifest::PluginPermission,
        host::plugin_host::PluginHost,
    },
};
use std::sync::{Arc, Mutex};

#[test]
fn panic_payload_kinds_are_classified_without_formatting() {
    let payload = std::panic::catch_unwind(|| panic!("secret")).unwrap_err();
    assert_eq!(super::panic_kind(payload.as_ref()), "str");
    let payload = std::panic::catch_unwind(|| panic!("{}", String::from("secret"))).unwrap_err();
    assert_eq!(super::panic_kind(payload.as_ref()), "string");
    let payload = std::panic::catch_unwind(|| std::panic::panic_any(7u8)).unwrap_err();
    assert_eq!(super::panic_kind(payload.as_ref()), "other");
}

#[derive(Default)]
struct Output(Mutex<Vec<String>>);
impl NotificationOutput for Output {
    fn emit(&self, _: &str, _: PluginNotificationLevel, message: &str) -> PluginResult<()> {
        self.0.lock().unwrap().push(message.into());
        Ok(())
    }
}
fn install(dir: &crate::test_support::TempDir, id: &str, source: &str) {
    let path = dir.join("plugins").join(id);
    std::fs::create_dir_all(&path).unwrap();
    let m = manifest(id);
    std::fs::write(path.join("manifest.json"), serde_json::to_vec(&m).unwrap()).unwrap();
    std::fs::write(path.join("index.js"), source).unwrap();
}
fn approve(dir: &crate::test_support::TempDir, ids: &[&str]) {
    let plugins=ids.iter().map(|id| (id.to_string(),serde_json::json!({"enabled":true,"permissions":manifest(id).permissions,"resyncIntervalSecs":10}))).collect::<serde_json::Map<_,_>>();
    std::fs::write(
        dir.join("plugins.json"),
        serde_json::json!({"plugins":plugins}).to_string(),
    )
    .unwrap();
}
async fn wait_for(output: &Output, message: &str) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if output.0.lock().unwrap().iter().any(|m| m == message) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plugins_without_event_grants_can_start_reload_and_stop() {
    for declares_events in [false, true] {
        let dir = crate::test_support::TempDir::unique("plugin-without-events");
        let id = "test.no-events";
        install(
            &dir,
            id,
            r#"export default async h => {
          const context = await h.context.current();
          if (!(await h.identity.current(context)).userKey) throw Error('identity');
          for (const event of ['resync.required', 'submission.judged']) {
            let denied = false;
            try { h.events.on(event, async () => {}); } catch (_) { denied = true; }
            if (!denied) throw Error('event grant widened');
          }
          await h.notification.info('activated');
        }"#,
        );
        let mut m = manifest(id);
        let approved = m
            .permissions
            .iter()
            .copied()
            .filter(|p| *p != PluginPermission::EventsSubscribe)
            .collect::<Vec<_>>();
        if !declares_events {
            m.events.clear();
            m.permissions = approved.clone();
        }
        m.validate().unwrap();
        std::fs::write(
            dir.join("plugins").join(id).join("manifest.json"),
            serde_json::to_vec(&m).unwrap(),
        )
        .unwrap();
        std::fs::write(dir.join("plugins.json"), serde_json::json!({
            "plugins": {id: {"enabled": true, "permissions": approved, "resyncIntervalSecs": 10}}
        }).to_string()).unwrap();
        let host = Arc::new(PluginHost::new(Arc::new(CoreEventBus::new())));
        let manager = make_manager(
            dir.to_path_buf(),
            host.clone(),
            Arc::new(Source::default()),
            Arc::default(),
        );
        let output = Arc::new(Output::default());
        manager.set_output(output.clone());
        manager.start(id).await.unwrap();
        wait_for(&output, "activated").await;
        assert_eq!(host.subscriber_count(), 0);
        assert_eq!(manager.resync(id), Err(PluginError::Unavailable));
        manager.reload(id).await.unwrap();
        assert_eq!(host.subscriber_count(), 0);
        assert_eq!(output.0.lock().unwrap().len(), 2);
        manager.stop(id).await.unwrap();
        let statuses = manager.list().unwrap();
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].state, "stopped");
        assert_eq!(statuses[0].error, None);
    }
}

#[test]
fn list_distinguishes_missing_configuration_from_invalid_configuration() {
    let dir = crate::test_support::TempDir::unique("plugin-list-errors");
    let manager = make_manager(
        dir.to_path_buf(),
        Arc::new(PluginHost::new(Arc::new(CoreEventBus::new()))),
        Arc::new(Source::default()),
        Arc::default(),
    );
    assert!(manager.list().unwrap().is_empty());
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("plugins.json"), "{broken").unwrap();
    assert!(matches!(manager.list(), Err(PluginError::InvalidInput)));
    std::fs::write(
        dir.join("plugins.json"),
        serde_json::json!({
            "plugins": {"test.invalid": {"enabled": true, "resyncIntervalSecs": 1}}
        })
        .to_string(),
    )
    .unwrap();
    let invalid = manager.list().unwrap();
    assert_eq!(invalid[0].state, "failed");
    assert_eq!(invalid[0].error, Some(PluginError::LimitExceeded));
    approve(&dir, &["test.configured"]);
    let statuses = manager.list().unwrap();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].id, "test.configured");
    assert_eq!(statuses[0].state, "stopped");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_reload_retains_diagnostics_and_a_later_reload_recovers() {
    let dir = crate::test_support::TempDir::unique("plugin-reload-errors");
    let id = "test.reload";
    install(&dir, id, "export default async () => {};");
    approve(&dir, &[id]);
    let host = Arc::new(PluginHost::new(Arc::new(CoreEventBus::new())));
    let manager = make_manager(
        dir.to_path_buf(),
        host.clone(),
        Arc::new(Source::default()),
        Arc::default(),
    );
    manager.set_output(Arc::new(Output::default()));
    manager.start(id).await.unwrap();
    let mut invalid = manifest(id);
    invalid.entry = "missing.js".into();
    std::fs::write(
        dir.join("plugins").join(id).join("manifest.json"),
        serde_json::to_vec(&invalid).unwrap(),
    )
    .unwrap();
    assert_eq!(manager.reload(id).await, Err(PluginError::NotFound));
    assert_eq!(host.subscriber_count(), 0);
    let statuses = manager.list().unwrap();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].state, "failed");
    assert_eq!(statuses[0].error, Some(PluginError::NotFound));
    assert_eq!(
        serde_json::to_value(&statuses[0]).unwrap()["error"],
        serde_json::json!({"code": "not_found"})
    );
    install(&dir, id, "export default async () => {};");
    manager.reload(id).await.unwrap();
    manager.stop(id).await.unwrap();
    let statuses = manager.list().unwrap();
    assert_eq!(statuses[0].state, "stopped");
    assert_eq!(statuses[0].error, None);
}

#[tokio::test]
async fn failed_explicit_start_retains_diagnostics() {
    let dir = crate::test_support::TempDir::unique("plugin-start-errors");
    std::fs::create_dir_all(&dir).unwrap();
    approve(&dir, &["test.missing"]);
    let manager = make_manager(
        dir.to_path_buf(),
        Arc::new(PluginHost::new(Arc::new(CoreEventBus::new()))),
        Arc::new(Source::default()),
        Arc::default(),
    );
    assert_eq!(
        manager.start("test.missing").await,
        Err(PluginError::Unavailable)
    );
    let statuses = manager.list().unwrap();
    assert_eq!(statuses[0].state, "failed");
    assert_eq!(statuses[0].error, Some(PluginError::Unavailable));
}

#[tokio::test]
async fn failure_before_worker_spawn_emits_only_redacted_diagnostics() {
    use std::io::Write;
    use tracing::instrument::WithSubscriber;
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
    // tracing 的 callsite Interest 缓存是进程级全局状态：当进程内只注册了一个
    // Dispatch 时，callsite 首次命中会经「全局默认 dispatcher（NoSubscriber）」
    // 评估 Interest 并缓存为 never——若并行的其他测试先命中 `start_inner` 里
    // 同一个 warn! callsite，本测试的事件会被永久跳过（缓冲区为空）。注册第二个
    // 对 WARN 级别返回 always 的 Dispatch，令 Interest 评估遍历真实注册的
    // dispatcher（含下方捕获 subscriber），从根上消除该竞态。
    let _sink_dispatch = tracing::dispatcher::Dispatch::new(
        tracing_subscriber::fmt()
            .with_ansi(false)
            .with_writer(std::io::sink)
            .finish(),
    );
    let dir = crate::test_support::TempDir::unique("plugin-start-log-private-path");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("plugins.json"), "private malformed configuration").unwrap();
    let manager = make_manager(
        dir.to_path_buf(),
        Arc::new(PluginHost::new(Arc::new(CoreEventBus::new()))),
        Arc::new(Source::default()),
        Arc::default(),
    );
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let writer = Capture(buffer.clone());
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    let subscriber = tracing::Dispatch::new(subscriber);
    assert_eq!(
        manager
            .start("test.diagnostic")
            .with_subscriber(subscriber.clone())
            .await,
        Err(PluginError::InvalidInput)
    );
    assert_eq!(
        manager
            .start("test.diagnostic\nFORGED\rENTRY")
            .with_subscriber(subscriber.clone())
            .await,
        Err(PluginError::InvalidInput)
    );
    assert_eq!(
        manager
            .start(&"a".repeat(129))
            .with_subscriber(subscriber)
            .await,
        Err(PluginError::InvalidInput)
    );
    let log = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
    assert!(log.contains("worker.start"));
    assert!(log.contains("test.diagnostic"));
    assert!(log.contains("InvalidInput"));
    assert!(!log.contains("private malformed configuration"));
    assert!(!log.contains("plugin-start-log-private-path"));
    assert!(!log.contains("test.diagnostic\nFORGED"));
    assert!(log.contains("test.diagnostic\\nFORGED\\rENTRY"));
    assert!(log.contains("<invalid>"));
    assert!(!log.contains(&"a".repeat(129)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_js_queries_cas_notifications_events_reload_and_isolated_failure() {
    let dir = crate::test_support::TempDir::unique("plugin-manager");
    install(
        &dir,
        "test.good",
        r#"
        export default async function activate(h, config) {
          if (!Object.isFrozen(config)) throw Error('mutable config');
          const context=await h.context.current();
          const identity=await h.identity.current(context);
          if (!identity.userKey || identity.token) throw Error('identity');
          const contest=await h.contest.current(context);
          if (contest.contestId !== 'contest' || await h.contest.get(context,'other') !== null) throw Error('contest');
          const history=await h.submission.list(context,{contestId:'contest',limit:100});
          if (history.nextCursor !== null) throw Error('history');
          if (await h.submission.get(context,'contest','missing') !== null) throw Error('metadata');
          try { await h.http.request(context,{method:'GET',url:'https://example.com'}); throw Error('network widened'); }
          catch(error) { if (error.code !== 'permission_denied') throw error; }
          await h.storage.set('temporary',null);await h.storage.delete('temporary');
          if ((await h.storage.get('temporary')).state !== 'missing') throw Error('delete');
          const acquired=await h.storage.compareAndSet('fixture',{state:'missing'},{count:1});
          await h.notification.info(acquired ? 'created' : 'retained');
          h.events.on('submission.judged', async event => {
            await h.storage.set('event',event.submissionId);
            await h.notification.success('event:'+event.submissionId);
          });
          let off=h.events.on('resync.required',async () => { off(); await h.notification.info('resync'); });
        }
    "#,
    );
    install(
        &dir,
        "test.bad",
        "export default async function () { throw Error('private stack'); }",
    );
    approve(&dir, &["test.good", "test.bad"]);
    let bus = Arc::new(CoreEventBus::new());
    let host = Arc::new(PluginHost::new(bus.clone()));
    let consumer = host.start().unwrap();
    let manager = make_manager(
        dir.to_path_buf(),
        host.clone(),
        Arc::new(Source::default()),
        Arc::new(AccessRevision::default()),
    );
    let output = Arc::new(Output::default());
    manager.set_output(output.clone());
    manager.start("test.good").await.unwrap();
    wait_for(&output, "created").await;
    wait_for(&output, "resync").await;
    assert!(manager.start("test.bad").await.is_err());
    bus.publish(CoreEvent::SubmissionJudged {
        submission_id: "id".into(),
        status: "Accepted".into(),
    });
    wait_for(&output, "event:id").await;
    manager.reload("test.good").await.unwrap();
    wait_for(&output, "retained").await;
    manager.stop("test.good").await.unwrap();
    manager.stop("test.bad").await.unwrap();
    assert_eq!(host.subscriber_count(), 0);
    assert!(manager.list().unwrap().iter().all(|p| p.state != "running"));
    consumer.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_interrupts_synchronous_handler_and_releases_worker() {
    let dir = crate::test_support::TempDir::unique("plugin-stop-loop");
    install(
        &dir,
        "test.loop",
        "export default async h => { h.events.on('resync.required',async()=>{for (;;) {}}); }",
    );
    approve(&dir, &["test.loop"]);
    let host = Arc::new(PluginHost::new(Arc::new(CoreEventBus::new())));
    let manager = make_manager(
        dir.to_path_buf(),
        host.clone(),
        Arc::new(Source::default()),
        Arc::default(),
    );
    manager.set_output(Arc::new(Output::default()));
    manager.start("test.loop").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    manager.stop("test.loop").await.unwrap();
    assert_eq!(host.subscriber_count(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_cancels_an_activation_waiting_on_a_never_resolved_promise() {
    let dir = crate::test_support::TempDir::unique("plugin-stop-promise");
    install(
        &dir,
        "test.promise",
        "export default async () => { await new Promise(()=>{}); }",
    );
    approve(&dir, &["test.promise"]);
    let host = Arc::new(PluginHost::new(Arc::new(CoreEventBus::new())));
    let manager = Arc::new(make_manager(
        dir.to_path_buf(),
        host.clone(),
        Arc::new(Source::default()),
        Arc::default(),
    ));
    manager.set_output(Arc::new(Output::default()));
    let pending = manager.clone();
    let start = tokio::spawn(async move { pending.start("test.promise").await });
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while !manager
            .list()
            .unwrap()
            .iter()
            .any(|p| p.state == "starting")
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    manager.stop("test.promise").await.unwrap();
    assert!(start.await.unwrap().is_err());
    assert_eq!(host.subscriber_count(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn periodic_resync_reconciles_without_new_business_events() {
    let dir = crate::test_support::TempDir::unique("plugin-periodic");
    install(&dir, "test.periodic", "export default async h => { let n=0; h.events.on('resync.required', async()=>{ if (++n === 2) await h.notification.info('periodic'); }); }");
    approve(&dir, &["test.periodic"]);
    let host = Arc::new(PluginHost::new(Arc::new(CoreEventBus::new())));
    let manager = make_manager(
        dir.to_path_buf(),
        host,
        Arc::new(Source::default()),
        Arc::default(),
    )
    .with_test_timing(
        std::time::Duration::from_secs(12),
        std::time::Duration::from_millis(30),
    );
    let output = Arc::new(Output::default());
    manager.set_output(output.clone());
    manager.start("test.periodic").await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(13), async {
        loop {
            if output
                .0
                .lock()
                .unwrap()
                .iter()
                .any(|text| text == "periodic")
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    manager.stop("test.periodic").await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bridge_limits_apply_before_a_batch_of_js_promises_is_polled() {
    let dir = crate::test_support::TempDir::unique("plugin-bridge-budget");
    install(
        &dir,
        "test.budget",
        r#"export default async h => {
      const calls = Array.from({length: 64}, () => h.storage.get('key'));
      const results = await Promise.allSettled(calls);
      if (results.filter(r => r.status === 'fulfilled').length !== 16) throw Error('budget');
      if (results.filter(r => r.status === 'rejected').some(r => r.reason.code !== 'limit_exceeded')) throw Error('error contract');
      await h.notification.info('budget checked');
    }"#,
    );
    approve(&dir, &["test.budget"]);
    let host = Arc::new(PluginHost::new(Arc::new(CoreEventBus::new())));
    let manager = make_manager(
        dir.to_path_buf(),
        host,
        Arc::new(Source::default()),
        Arc::default(),
    );
    let output = Arc::new(Output::default());
    manager.set_output(output.clone());
    manager.start("test.budget").await.unwrap();
    wait_for(&output, "budget checked").await;
    manager.stop("test.budget").await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slow_activation_does_not_subscribe_until_it_can_receive_initial_resync() {
    struct Gate {
        entered: tokio::sync::Notify,
        released: (Mutex<bool>, std::sync::Condvar),
    }
    impl NotificationOutput for Gate {
        fn emit(&self, _: &str, _: PluginNotificationLevel, _: &str) -> PluginResult<()> {
            self.entered.notify_one();
            let (lock, ready) = &self.released;
            let _guard = ready
                .wait_while(lock.lock().unwrap(), |released| !*released)
                .unwrap();
            Ok(())
        }
    }
    let dir = crate::test_support::TempDir::unique("plugin-slow-activate");
    install(&dir, "test.slow", "export default async h => { h.events.on('resync.required',async()=>{}); await h.notification.info('activating'); }");
    approve(&dir, &["test.slow"]);
    let host = Arc::new(PluginHost::new(Arc::new(CoreEventBus::new())));
    let manager = Arc::new(make_manager(
        dir.to_path_buf(),
        host.clone(),
        Arc::new(Source::default()),
        Arc::default(),
    ));
    let gate = Arc::new(Gate {
        entered: tokio::sync::Notify::new(),
        released: (Mutex::new(false), std::sync::Condvar::new()),
    });
    manager.set_output(gate.clone());
    let pending = manager.clone();
    let start = tokio::spawn(async move { pending.start("test.slow").await });
    gate.entered.notified().await;
    let subscribers_during_activation = host.subscriber_count();
    for _ in 0..256 {
        host.dispatch(&CoreEvent::SubmissionJudged {
            submission_id: "id".into(),
            status: "Accepted".into(),
        });
    }
    *gate.released.0.lock().unwrap() = true;
    gate.released.1.notify_one();
    start.await.unwrap().unwrap();
    assert_eq!(subscribers_during_activation, 0);
    assert_eq!(host.subscriber_count(), 1);
    assert_eq!(manager.list().unwrap()[0].error, None);
    manager.stop("test.slow").await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn start_timeout_cancels_and_joins_worker_and_duplicate_start_keeps_history_clean() {
    let dir = crate::test_support::TempDir::unique("plugin-start-timeout");
    install(
        &dir,
        "test.timeout",
        "export default async () => { await new Promise(()=>{}); }",
    );
    approve(&dir, &["test.timeout"]);
    let host = Arc::new(PluginHost::new(Arc::new(CoreEventBus::new())));
    let manager = make_manager(
        dir.to_path_buf(),
        host.clone(),
        Arc::new(Source::default()),
        Arc::default(),
    )
    .with_test_timing(
        std::time::Duration::from_millis(30),
        std::time::Duration::from_secs(60),
    );
    manager.set_output(Arc::new(Output::default()));
    assert_eq!(
        manager.start("test.timeout").await,
        Err(PluginError::Timeout)
    );
    assert_eq!(host.subscriber_count(), 0);
    let status = manager.list().unwrap();
    assert_eq!(status[0].state, "failed");
    assert_eq!(status[0].error, Some(PluginError::Timeout));
    let manager = manager.with_test_timing(
        std::time::Duration::from_secs(12),
        std::time::Duration::from_secs(60),
    );
    install(&dir, "test.timeout", "export default async () => {};");
    manager.start("test.timeout").await.unwrap();
    assert_eq!(
        manager.start("test.timeout").await,
        Err(PluginError::Unavailable)
    );
    manager.stop("test.timeout").await.unwrap();
    let status = manager.list().unwrap();
    assert_eq!(status[0].state, "stopped");
    assert_eq!(status[0].error, None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn syntax_and_runtime_errors_are_stable_script_errors() {
    for script in [
        "export default !!!",
        "export default async () => { throw Error('PRIVATE stack/path'); }",
    ] {
        let dir = crate::test_support::TempDir::unique("plugin-script-errors");
        install(&dir, "test.script", script);
        approve(&dir, &["test.script"]);
        let manager = make_manager(
            dir.to_path_buf(),
            Arc::new(PluginHost::new(Arc::new(CoreEventBus::new()))),
            Arc::new(Source::default()),
            Arc::default(),
        );
        manager.set_output(Arc::new(Output::default()));
        assert_eq!(
            manager.start("test.script").await,
            Err(PluginError::ScriptError)
        );
        manager.stop("test.script").await.unwrap();
        assert_eq!(
            serde_json::to_value(&manager.list().unwrap()[0]).unwrap()["error"],
            serde_json::json!({"code":"script_error"})
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_real_plugins_share_the_manager_provider_budget() {
    use std::sync::atomic::Ordering;
    let dir = crate::test_support::TempDir::unique("plugins-shared-budget");
    install(&dir, "test.first", "export default async h => { const c=await h.context.current(); await h.contest.current(c); await h.contest.current(c); }");
    install(
        &dir,
        "test.second",
        r#"export default async h => {
      const c=await h.context.current(); await h.contest.current(c);
      try { await h.contest.current(c); throw Error('shared budget bypassed'); }
      catch (e) { if (e.code !== 'limit_exceeded') throw e; }
      await h.notification.info('limited');
    }"#,
    );
    approve(&dir, &["test.first", "test.second"]);
    let source = Arc::new(Source::default());
    let manager = make_manager(
        dir.to_path_buf(),
        Arc::new(PluginHost::new(Arc::new(CoreEventBus::new()))),
        source.clone(),
        Arc::default(),
    )
    .with_test_query_budget(3);
    let output = Arc::new(Output::default());
    manager.set_output(output.clone());
    manager.start("test.first").await.unwrap();
    manager.start("test.second").await.unwrap();
    wait_for(&output, "limited").await;
    assert_eq!(source.contest_calls.load(Ordering::SeqCst), 3);
    manager.stop("test.first").await.unwrap();
    manager.stop("test.second").await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_json_arguments_reject_with_stable_errors_without_failing_worker() {
    let dir = crate::test_support::TempDir::unique("plugin-json-errors");
    install(
        &dir,
        "test.json-errors",
        r#"export default async h => {
      const cyclic = {}; cyclic.self = cyclic;
      for (const value of [cyclic, 1n, {toJSON() {throw Error('PRIVATE_SERIALIZATION_REASON')}}]) {
        let caught = false;
        try { await h.storage.set('invalid', value); }
        catch (e) {
          if (e.code !== 'invalid_input' || Object.keys(e).join() !== 'code' || !Object.isFrozen(e)) throw Error('bad error contract');
          caught = true;
        }
        if (!caught) throw Error('invalid input accepted');
      }
      if ((await h.storage.get('invalid')).state !== 'missing') throw Error('invalid write reached host');
      await h.storage.set('valid', 'ok');
      if ((await h.storage.get('valid')).value !== 'ok') throw Error('bridge stopped working');
      await h.notification.info('json-errors-caught');
    }"#,
    );
    approve(&dir, &["test.json-errors"]);
    let manager = make_manager(
        dir.to_path_buf(),
        Arc::new(PluginHost::new(Arc::new(CoreEventBus::new()))),
        Arc::new(Source::default()),
        Arc::default(),
    );
    let output = Arc::new(Output::default());
    manager.set_output(output.clone());
    manager.start("test.json-errors").await.unwrap();
    wait_for(&output, "json-errors-caught").await;
    assert_eq!(manager.list().unwrap()[0].error, None);
    manager.stop("test.json-errors").await.unwrap();
    assert_eq!(manager.list().unwrap()[0].error, None);
}
