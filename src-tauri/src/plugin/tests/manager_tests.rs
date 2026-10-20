use super::support::*;
use crate::{
    core::{
        access::AccessRevision,
        event::{core_event::CoreEvent, core_event_bus::CoreEventBus},
    },
    plugin::{
        api::{notification::PluginNotificationLevel, *},
        execution_api::NotificationOutput,
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
    assert!(manager.list().iter().all(|p| p.state != "running"));
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
        while !manager.list().iter().any(|p| p.state == "starting") {
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
