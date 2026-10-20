use super::*;
use crate::core::event::core_event_bus::CoreEventBus;
use crate::plugin::tests::support::Source;

#[test]
fn diagnostic_history_is_bounded_and_removed_configuration_is_pruned() {
    let dir = crate::test_support::TempDir::unique("plugin-diagnostic-history");
    let manager = PluginManager::new(
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
