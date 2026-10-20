use super::support::*;
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
        manager::PluginManager,
    },
};
use std::sync::{Arc, Mutex};

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
        let manager = PluginManager::new(
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
    let manager = PluginManager::new(
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
    assert!(matches!(manager.list(), Err(PluginError::LimitExceeded)));
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
    let manager = PluginManager::new(
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
    let manager = PluginManager::new(
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
    let manager = PluginManager::new(
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
    let manager = PluginManager::new(
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
    let manager = Arc::new(PluginManager::new(
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
    let manager = PluginManager::new(
        dir.to_path_buf(),
        host,
        Arc::new(Source::default()),
        Arc::default(),
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
    let manager = PluginManager::new(
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
