use super::*;
use std::sync::Arc;

use crate::core::entity::config::AppConfig;
use crate::core::event::core_event::CoreEvent;
use crate::core::event::core_event_bus::CoreEventBus;
use crate::infra::fs_config_repo::FsConfigRepository;
use crate::infra::storage::Storage;
use crate::test_support::{Guarded, TempDir};

/// 订阅总线并返回 receiver（事件断言用；`try_recv` 同步取，无需 async 用例）。
fn subscribe(bus: &Arc<CoreEventBus>) -> tokio::sync::broadcast::Receiver<CoreEvent> {
    bus.subscribe()
}

fn test_config_service(path: &str) -> (Guarded<ConfigService<FsConfigRepository>>, Arc<Storage>) {
    let dir = TempDir::named(&format!("hinina-test-cfg-{}", path));
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let repo = Arc::new(FsConfigRepository::new(Arc::clone(&storage), "config.json"));
    let event_bus = Arc::new(CoreEventBus::new());
    (
        Guarded::new(ConfigService::new(repo, event_bus), dir),
        storage,
    )
}

#[test]
fn new_loads_existing_config() {
    let dir = TempDir::named("hinina-test-cfg-existing");
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let repo = Arc::new(FsConfigRepository::new(Arc::clone(&storage), "config.json"));

    // save a custom config to disk before creating the service
    let mut custom = AppConfig::default();
    custom.theme.theme_name = "light".into();
    custom.editor.font_size = 18;
    repo.save_config(&custom).unwrap();

    let service = ConfigService::new(repo, Arc::new(CoreEventBus::new()));
    let loaded = service.get();
    assert_eq!(loaded.theme.theme_name, "light");
    assert_eq!(loaded.editor.font_size, 18);
}

#[test]
fn new_uses_defaults_when_no_config() {
    let (service, _storage) = test_config_service("defaults");

    let cfg = service.get();
    assert_eq!(cfg.theme.theme_name, "light");
    assert_eq!(cfg.editor.font_size, 14);
    assert_eq!(cfg.editor.default_language, "C++");
    // OJ 选择是应用级状态（oj.active），默认 HOJ；实例清单默认单 HOJ
    assert_eq!(cfg.oj.active, "HOJ");
    assert_eq!(cfg.oj.instances.len(), 1);
    assert_eq!(cfg.oj.instances[0].id, "HOJ");
}

#[test]
fn new_normalizes_legacy_config_from_disk() {
    // P55：上一版落盘的 Monaco id 'cpp' / dark / 0.45 在加载路径一次性归一
    let dir = TempDir::named("hinina-test-cfg-legacy-normalize");
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    storage
        .write_string(
            "config.json",
            r#"{
                "editor": { "defaultLanguage": "cpp" },
                "theme": { "themeName": "dark", "editorTheme": "vs-dark" },
                "layout": { "splitRatio": 0.45 }
            }"#,
        )
        .unwrap();
    let repo = Arc::new(FsConfigRepository::new(Arc::clone(&storage), "config.json"));

    let service = ConfigService::new(repo, Arc::new(CoreEventBus::new()));
    let cfg = service.get();
    assert_eq!(cfg.editor.default_language, "C++");
    assert_eq!(cfg.theme.theme_name, "light");
    assert_eq!(cfg.theme.editor_theme, "vs");
    assert_eq!(cfg.layout.split_ratio, 0.48);
}

#[test]
fn update_persists_changes() {
    let dir = TempDir::named("hinina-test-cfg-update-persist");
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let repo = Arc::new(FsConfigRepository::new(Arc::clone(&storage), "config.json"));

    {
        let service = ConfigService::new(repo, Arc::new(CoreEventBus::new()));
        service
            .update(|cfg| {
                cfg.theme.theme_name = "light".into();
                cfg.editor.tab_size = 2;
            })
            .unwrap();
    }

    // create a new ConfigService — should pick up persisted changes
    let repo2 = Arc::new(FsConfigRepository::new(Arc::clone(&storage), "config.json"));
    let service2 = ConfigService::new(repo2, Arc::new(CoreEventBus::new()));
    let cfg = service2.get();
    assert_eq!(cfg.theme.theme_name, "light");
    assert_eq!(cfg.editor.tab_size, 2);
}

#[test]
fn reload_from_disk_overwrites_memory() {
    let (service, storage) = test_config_service("reload");

    // service loads defaults
    assert_eq!(service.get().theme.theme_name, "light");

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
fn reload_publishes_config_changed_event() {
    let dir = TempDir::named("hinina-test-cfg-reload-event");
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let repo = Arc::new(FsConfigRepository::new(Arc::clone(&storage), "config.json"));
    let event_bus = Arc::new(CoreEventBus::new());
    let service = ConfigService::new(repo, Arc::clone(&event_bus));
    let mut rx = subscribe(&event_bus);

    service.reload().unwrap();

    assert_eq!(
        rx.try_recv().expect("reload 成功后应发布 ConfigChanged"),
        CoreEvent::ConfigChanged
    );
}

/// `update` 也必须发布 `ConfigChanged`（2026-09-21 复核发现的生产断链）。
///
/// 生产链路的配置变更入口是 `update`（`update_config` 命令、主题切换、`switch_oj`
/// 的持久化都走它），而 `reload_config` 命令在前端**没有任何调用方**（只有 spec 的
/// mock）。只在 `reload` 里发布事件，观察者永远等不到。
///
/// 注意：需要「配置变更后必须完成」的动作（auto-save 启停同步）**不依赖本事件** ——
/// 由 `update_config` / `reload_config` 命令显式调用（见 `commands/config_cmd.rs`）。
#[test]
fn update_publishes_config_changed_event() {
    let dir = TempDir::named("hinina-test-cfg-update-event");
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let repo = Arc::new(FsConfigRepository::new(Arc::clone(&storage), "config.json"));
    let event_bus = Arc::new(CoreEventBus::new());
    let service = ConfigService::new(repo, Arc::clone(&event_bus));
    let mut rx = subscribe(&event_bus);

    service.update(|cfg| cfg.editor.auto_save = false).unwrap();

    assert_eq!(
        rx.try_recv().expect("update 成功后应发布 ConfigChanged"),
        CoreEvent::ConfigChanged
    );
}

/// 事件到达时配置**确已落盘**：`ConfigChanged` 表示「新配置已生效」，
/// 观察者据此行动的前提是磁盘真值已经更新（否则重启后配置回退，而事件已宣告成功）。
#[test]
fn config_changed_is_published_only_after_persist() {
    let dir = TempDir::named("hinina-test-cfg-event-after-persist");
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let repo = Arc::new(FsConfigRepository::new(Arc::clone(&storage), "config.json"));
    let event_bus = Arc::new(CoreEventBus::new());
    let service = ConfigService::new(repo, Arc::clone(&event_bus));
    let mut rx = subscribe(&event_bus);

    service.update(|cfg| cfg.editor.auto_save = false).unwrap();

    assert_eq!(rx.try_recv().unwrap(), CoreEvent::ConfigChanged);
    let raw = storage.read_to_string("config.json").unwrap();
    let persisted: AppConfig = serde_json::from_str(&raw).unwrap();
    assert!(
        !persisted.editor.auto_save,
        "事件到达时磁盘上的配置必须已是新值"
    );
}

/// 落盘失败时**不发布**：磁盘与内存已不一致，让观察者按「新配置已生效」行动会掩盖问题。
#[test]
fn update_failure_does_not_publish_config_changed_event() {
    let dir = TempDir::named("hinina-test-cfg-update-event-fail");
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let repo = Arc::new(FsConfigRepository::new(Arc::clone(&storage), "config.json"));
    let event_bus = Arc::new(CoreEventBus::new());
    let service = ConfigService::new(repo, Arc::clone(&event_bus));
    let mut rx = subscribe(&event_bus);

    // 构造后 config.json 已存在（构造函数会落盘默认值）：先删掉再占位成目录，
    // 使后续写入必然失败（确定性，不依赖权限或只读卷）
    std::fs::remove_file(dir.join("config.json")).unwrap();
    std::fs::create_dir_all(dir.join("config.json")).unwrap();

    assert!(service.update(|cfg| cfg.editor.auto_save = false).is_err());
    assert!(rx.try_recv().is_err(), "持久化失败不得发布 ConfigChanged");
}
