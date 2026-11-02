use super::*;

#[test]
fn connection_changes_revoke_scope_even_when_persistence_fails() {
    let dir = TempDir::unique("config-access-fail");
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let repo = Arc::new(FsConfigRepository::new(storage, "config.json"));
    let access = Arc::new(crate::core::access::AccessRevision::default());
    let service = ConfigService::new(repo, Arc::new(CoreEventBus::new()), access.clone());
    let before = access.snapshot().unwrap();
    service.update(|cfg| cfg.editor.font_size += 1).unwrap();
    assert!(!before.cancellation.is_cancelled());
    std::fs::remove_file(dir.join("config.json")).unwrap();
    std::fs::create_dir(dir.join("config.json")).unwrap();
    assert!(service
        .update(|cfg| cfg.oj.active = "different".into())
        .is_err());
    assert!(before.cancellation.is_cancelled());
    assert_ne!(before.revision, access.snapshot().unwrap().revision);
}
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
        Guarded::new(ConfigService::new(repo, event_bus, Arc::default()), dir),
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

    let service = ConfigService::new(repo, Arc::new(CoreEventBus::new()), Arc::default());
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

    let service = ConfigService::new(repo, Arc::new(CoreEventBus::new()), Arc::default());
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
        let service = ConfigService::new(repo, Arc::new(CoreEventBus::new()), Arc::default());
        service
            .update(|cfg| {
                cfg.theme.theme_name = "light".into();
                cfg.editor.tab_size = 2;
            })
            .unwrap();
    }

    // create a new ConfigService — should pick up persisted changes
    let repo2 = Arc::new(FsConfigRepository::new(Arc::clone(&storage), "config.json"));
    let service2 = ConfigService::new(repo2, Arc::new(CoreEventBus::new()), Arc::default());
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
    let service = ConfigService::new(repo, Arc::clone(&event_bus), Arc::default());
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
    let service = ConfigService::new(repo, Arc::clone(&event_bus), Arc::default());
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
    let service = ConfigService::new(repo, Arc::clone(&event_bus), Arc::default());
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
    let service = ConfigService::new(repo, Arc::clone(&event_bus), Arc::default());
    let mut rx = subscribe(&event_bus);

    // 构造后 config.json 已存在（构造函数会落盘默认值）：先删掉再占位成目录，
    // 使后续写入必然失败（确定性，不依赖权限或只读卷）
    std::fs::remove_file(dir.join("config.json")).unwrap();
    std::fs::create_dir_all(dir.join("config.json")).unwrap();

    assert!(service.update(|cfg| cfg.editor.auto_save = false).is_err());
    assert!(rx.try_recv().is_err(), "持久化失败不得发布 ConfigChanged");
}

/// 可延迟的配置仓库：第一次 `save_config` 阻塞到测试放行，用于构造
/// 「旧快照后落盘」的时序回归。
struct DelayedSaveRepo {
    inner: FsConfigRepository,
    started: std::sync::mpsc::Sender<()>,
    release: std::sync::Mutex<std::sync::mpsc::Receiver<()>>,
    saves: std::sync::Mutex<Vec<AppConfig>>,
    /// 武装后才延迟：`ConfigService::new` 在首次启动时会立即保存默认配置，
    /// 构造期不能阻塞
    armed: std::sync::atomic::AtomicBool,
}

impl DelayedSaveRepo {
    fn arm(&self) {
        self.armed.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

impl ConfigRepository for DelayedSaveRepo {
    fn load_config<T: serde::de::DeserializeOwned>(&self) -> AppResult<T> {
        self.inner.load_config()
    }

    fn save_config<T: serde::Serialize>(&self, config: &T) -> AppResult<()> {
        if self.armed.swap(false, std::sync::atomic::Ordering::SeqCst) {
            let _ = self.started.send(());
            let release = self.release.lock().unwrap_or_else(|e| e.into_inner());
            let _ = release.recv();
        }
        let value: AppConfig =
            serde_json::from_value(serde_json::to_value(config).expect("序列化配置"))
                .expect("配置可反序列化");
        self.saves
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(value);
        self.inner.save_config(config)
    }

    fn config_exists(&self) -> bool {
        self.inner.config_exists()
    }
}

/// 写事务锁：先克隆的旧配置不得后于新配置落盘（内存/磁盘/Registry 三者一致）。
///
/// 时序：T1 的 updater 先跑（active 仍为 A），其 save 被仓库阻塞；T2 随后尝试
/// 把 active 切到 B。**确定性握手**：T2 在调用 update 前用
/// `write_txn_is_held` 探测 T1 是否持有写事务；持有则 T2 必然阻塞，主线程直接
/// 放行 T1；未持有（移除锁的变异）则等 T2 真正写完 B 后再放行 T1，让旧交错
/// 必然发生——不再依赖 50ms sleep 猜调度。
#[test]
fn stale_update_cannot_overwrite_disk_after_a_concurrent_switch() {
    let dir = TempDir::unique("config-write-txn");
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let (probe_tx, probe_rx) = std::sync::mpsc::channel::<bool>();
    let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
    let repo = Arc::new(DelayedSaveRepo {
        inner: FsConfigRepository::new(Arc::clone(&storage), "config.json"),
        started: started_tx,
        release: std::sync::Mutex::new(release_rx),
        saves: std::sync::Mutex::new(Vec::new()),
        armed: std::sync::atomic::AtomicBool::new(false),
    });
    let service = Arc::new(ConfigService::new(
        repo.clone(),
        Arc::new(CoreEventBus::new()),
        Arc::default(),
    ));

    repo.arm(); // 只延迟下一次保存（构造期的默认配置保存不受影响）

    // T1：普通保存（旧 active=A），落盘被阻塞
    let t1 = {
        let service = Arc::clone(&service);
        std::thread::spawn(move || service.update(|cfg| cfg.editor.font_size = 20))
    };
    started_rx.recv().expect("T1 的保存应已开始并阻塞");

    // T2：切换 active=B（模拟 switch_oj 的持久化）
    let t2 = {
        let service = Arc::clone(&service);
        std::thread::spawn(move || {
            let held = service.write_txn_is_held();
            probe_tx.send(held).expect("回报写事务持有状态");
            let result = service.update(|cfg| cfg.oj.active = "B".into());
            if !held {
                // 锁缺失：T2 已完整落盘，通知主线程再放行 T1
                let _ = done_tx.send(());
            }
            result
        })
    };
    let held = probe_rx.recv().expect("T2 应已发起 update");
    if !held {
        done_rx.recv().expect("无锁路径下 T2 应先完成落盘");
    }
    // 放行 T1
    let _ = release_tx.send(());

    t1.join().unwrap().expect("T1 保存成功");
    t2.join().unwrap().expect("T2 保存成功");

    let saves = repo.saves.lock().unwrap_or_else(|e| e.into_inner());
    // 构造期默认配置 + T1 + T2
    assert_eq!(saves.len(), 3);
    assert_eq!(
        saves.last().unwrap().oj.active,
        "B",
        "磁盘最后写入必须是新 active"
    );
    assert_eq!(service.get().oj.active, "B", "内存与磁盘一致");
}
