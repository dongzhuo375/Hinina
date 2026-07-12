use super::*;
use std::sync::Arc;

use crate::core::entity::config::AppConfig;
use crate::core::event::app_event::{AppEvent, SystemEvent};
use crate::core::event::event_bus::EventBus;
use crate::core::event::event_category::EventCategory;
use crate::infra::fs_config_repo::FsConfigRepository;
use crate::infra::storage::Storage;

fn test_config_service(path: &str) -> (ConfigService<FsConfigRepository>, Arc<Storage>) {
    let dir = std::env::temp_dir().join(format!("hinina-test-cfg-{}", path));
    let _ = std::fs::remove_dir_all(&dir);
    let storage = Arc::new(Storage::new(dir));
    let repo = Arc::new(FsConfigRepository::new(Arc::clone(&storage), "config.json"));
    let event_bus = Arc::new(EventBus::new());
    (ConfigService::new(repo, event_bus), storage)
}

#[test]
fn new_loads_existing_config() {
    let dir = std::env::temp_dir().join("hinina-test-cfg-existing");
    let _ = std::fs::remove_dir_all(&dir);
    let storage = Arc::new(Storage::new(dir));
    let repo = Arc::new(FsConfigRepository::new(Arc::clone(&storage), "config.json"));

    // save a custom config to disk before creating the service
    let mut custom = AppConfig::default();
    custom.theme.theme_name = "light".into();
    custom.editor.font_size = 18;
    repo.save_config(&custom).unwrap();

    let service = ConfigService::new(repo, Arc::new(EventBus::new()));
    let loaded = service.get();
    assert_eq!(loaded.theme.theme_name, "light");
    assert_eq!(loaded.editor.font_size, 18);
}

#[test]
fn new_uses_defaults_when_no_config() {
    let (service, _storage) = test_config_service("defaults");

    let cfg = service.get();
    assert_eq!(cfg.theme.theme_name, "dark");
    assert_eq!(cfg.editor.font_size, 14);
    assert_eq!(cfg.editor.default_language, "C++");
    assert_eq!(cfg.user.last_oj_type, "HOJ");
}

#[test]
fn update_persists_changes() {
    let dir = std::env::temp_dir().join("hinina-test-cfg-update-persist");
    let _ = std::fs::remove_dir_all(&dir);
    let storage = Arc::new(Storage::new(dir));
    let repo = Arc::new(FsConfigRepository::new(Arc::clone(&storage), "config.json"));

    {
        let service = ConfigService::new(repo, Arc::new(EventBus::new()));
        service
            .update(|cfg| {
                cfg.theme.theme_name = "light".into();
                cfg.editor.tab_size = 2;
            })
            .unwrap();
    }

    // create a new ConfigService — should pick up persisted changes
    let repo2 = Arc::new(FsConfigRepository::new(Arc::clone(&storage), "config.json"));
    let service2 = ConfigService::new(repo2, Arc::new(EventBus::new()));
    let cfg = service2.get();
    assert_eq!(cfg.theme.theme_name, "light");
    assert_eq!(cfg.editor.tab_size, 2);
}

#[test]
fn reload_from_disk_overwrites_memory() {
    let (service, storage) = test_config_service("reload");

    // service loads defaults
    assert_eq!(service.get().theme.theme_name, "dark");

    // modify config directly on disk, bypassing the service
    let repo = FsConfigRepository::new(Arc::clone(&storage), "config.json");
    let mut disk_cfg = AppConfig::default();
    disk_cfg.theme.theme_name = "high-contrast".into();
    disk_cfg.layout.sidebar_width = 320;
    repo.save_config(&disk_cfg).unwrap();

    // reload should overwrite in-memory config from disk
    let reloaded = service.reload().unwrap();
    assert_eq!(reloaded.theme.theme_name, "high-contrast");
    assert_eq!(reloaded.layout.sidebar_width, 320);
    assert_eq!(service.get().theme.theme_name, "high-contrast");
}

#[test]
fn reload_publishes_config_reloaded_event() {
    let dir = std::env::temp_dir().join("hinina-test-cfg-reload-event");
    let _ = std::fs::remove_dir_all(&dir);
    let storage = Arc::new(Storage::new(dir));
    let repo = Arc::new(FsConfigRepository::new(Arc::clone(&storage), "config.json"));
    let event_bus = Arc::new(EventBus::new());
    let service = ConfigService::new(repo, Arc::clone(&event_bus));

    let received = Arc::new(std::sync::Mutex::new(false));
    let received_clone = Arc::clone(&received);

    event_bus.subscribe(
        EventCategory::System,
        Arc::new(move |event| {
            if matches!(event, AppEvent::System(SystemEvent::ConfigReloaded)) {
                *received_clone.lock().unwrap() = true;
            }
        }),
    );

    service.reload().unwrap();
    assert!(*received.lock().unwrap(), "ConfigReloaded event should have been published");
}
