use super::*;
use crate::core::event::core_event_bus::CoreEventBus;
use crate::plugin::tests::support::{make_manager, Source};

#[test]
fn diagnostic_history_is_bounded_and_removed_configuration_is_pruned() {
    let dir = crate::test_support::TempDir::unique("plugin-diagnostic-history");
    let manager = make_manager(
        dir.to_path_buf(),
        Arc::new(PluginHost::new(Arc::new(CoreEventBus::new()))),
        Arc::new(Source::default()),
        Arc::default(),
    );
    for index in 0..300 {
        manager.remember_status(PluginStatus {
            id: format!("test.plugin-{index}"),
            state: "failed",
            error: Some(PluginError::NotFound),
        });
    }
    let history = manager.statuses.lock().unwrap();
    assert_eq!(history.len(), 128);
    assert_eq!(history.front().unwrap().id, "test.plugin-172");
    drop(history);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("plugins.json"),
        r#"{"plugins":{"test.plugin-299":{}}}"#,
    )
    .unwrap();
    let statuses = manager.list().unwrap();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].id, "test.plugin-299");
    assert_eq!(statuses[0].error, Some(PluginError::NotFound));
    assert_eq!(manager.statuses.lock().unwrap().len(), 1);
    std::fs::write(dir.join("plugins.json"), "{}").unwrap();
    assert!(manager.list().unwrap().is_empty());
    assert!(manager.statuses.lock().unwrap().is_empty());
}

#[tokio::test]
async fn manager_uses_injected_repository_and_capability_factory_without_filesystem() {
    use crate::plugin::{registry::*, tests::support::manifest};
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct MemoryRepository(Arc<AtomicUsize>);
    impl PluginRepository for MemoryRepository {
        fn configuration(&self) -> PluginResult<PluginConfiguration> {
            let approval = serde_json::from_value(serde_json::json!({"enabled":true})).unwrap();
            Ok(PluginConfiguration {
                plugins: BTreeMap::from([("test.memory".into(), approval)]),
                ..Default::default()
            })
        }
        fn load(&self, _: &str) -> PluginResult<LoadedPlugin> {
            self.0.fetch_add(1, Ordering::SeqCst);
            let m = manifest("test.memory");
            Ok(LoadedPlugin {
                grant: super::super::permission::PluginGrant::authorize(&m, &m.permissions)?,
                source: "ignored by fixture factory".into(),
                approval: self.configuration()?.plugins.remove("test.memory").unwrap(),
            })
        }
    }
    struct Factory(Arc<AtomicUsize>);
    impl PluginApiFactory for Factory {
        fn create(
            &self,
            _: Arc<ScopedQueries>,
            _: &PluginApproval,
            _: Arc<dyn NotificationOutput>,
        ) -> PluginResult<Arc<dyn PluginApi>> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(PluginError::Unsupported)
        }
    }
    struct Output;
    impl NotificationOutput for Output {
        fn emit(
            &self,
            _: &str,
            _: crate::plugin::api::notification::PluginNotificationLevel,
            _: &str,
        ) -> PluginResult<()> {
            Ok(())
        }
    }
    let loads = Arc::new(AtomicUsize::new(0));
    let creates = Arc::new(AtomicUsize::new(0));
    let manager = PluginManager::new(
        Arc::new(MemoryRepository(loads.clone())),
        Arc::new(Factory(creates.clone())),
        Arc::new(PluginHost::new(Arc::new(CoreEventBus::new()))),
        Arc::new(Source::default()),
        Arc::default(),
    );
    manager.set_output(Arc::new(Output));
    assert_eq!(
        manager.start("test.memory").await,
        Err(PluginError::Unsupported)
    );
    assert_eq!(loads.load(Ordering::SeqCst), 1);
    assert_eq!(creates.load(Ordering::SeqCst), 1);
    assert!(manager.workers.lock().unwrap().is_empty());
    assert_eq!(
        manager.list().unwrap()[0].error,
        Some(PluginError::Unsupported)
    );
}

#[tokio::test]
async fn timed_out_worker_is_reaped_after_native_work_finishes() {
    let dir = crate::test_support::TempDir::unique("plugin-reaper");
    let manager = make_manager(
        dir.to_path_buf(),
        Arc::new(PluginHost::new(Arc::new(CoreEventBus::new()))),
        Arc::new(Source::default()),
        Arc::default(),
    );
    let (release, blocked) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        blocked.recv().unwrap();
    });
    let stop = CancellationToken::new();
    stop.cancel();
    manager.workers.lock().unwrap().insert(
        "test.cleanup".into(),
        Worker {
            stop,
            thread,
            status: Arc::new(Mutex::new(PluginStatus {
                id: "test.cleanup".into(),
                state: "failed",
                error: Some(PluginError::Timeout),
            })),
        },
    );
    manager.reap_when_finished("test.cleanup");
    assert_eq!(manager.workers.lock().unwrap().len(), 1);
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while !manager.workers.lock().unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn stop_timeout_marks_worker_failed_and_reaps_it_after_exit() {
    let dir = crate::test_support::TempDir::unique("plugin-stop-timeout");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("plugins.json"), r#"{"plugins":{"test.stop":{}}}"#).unwrap();
    let manager = make_manager(
        dir.to_path_buf(),
        Arc::new(PluginHost::new(Arc::new(CoreEventBus::new()))),
        Arc::new(Source::default()),
        Arc::default(),
    )
    .with_test_stop_timeout(Duration::from_millis(5));
    let (release, blocked) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        blocked.recv().unwrap();
    });
    manager.workers.lock().unwrap().insert(
        "test.stop".into(),
        Worker {
            stop: CancellationToken::new(),
            thread,
            status: Arc::new(Mutex::new(PluginStatus {
                id: "test.stop".into(),
                state: "running",
                error: None,
            })),
        },
    );
    assert_eq!(manager.stop("test.stop").await, Err(PluginError::Timeout));
    assert_eq!(manager.list().unwrap()[0].state, "failed");
    {
        let worker = manager.workers.lock().unwrap();
        assert!(worker["test.stop"].stop.is_cancelled());
        assert_eq!(worker["test.stop"].status.lock().unwrap().state, "failed");
        assert_eq!(
            worker["test.stop"].status.lock().unwrap().error,
            Some(PluginError::Timeout)
        );
    }
    assert_eq!(
        manager.start("test.stop").await,
        Err(PluginError::Unavailable)
    );
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while !manager.workers.lock().unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        manager.statuses.lock().unwrap().back().unwrap().error,
        Some(PluginError::Timeout)
    );
    assert_eq!(manager.list().unwrap()[0].error, Some(PluginError::Timeout));
}
