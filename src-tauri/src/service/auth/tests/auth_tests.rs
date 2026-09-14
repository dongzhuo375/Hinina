use super::*;

use std::sync::Arc;

use crate::core::event::event_category::EventCategory;
use crate::core::provider::oj_type::OJType;
use crate::infra::provider_registry_impl::ProviderRegistryImpl;

/// 构造基于独立临时目录的 AuthService 及其依赖。
fn make_service(test_name: &str) -> (AuthService, Arc<EventBus>, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("hinina-test-auth-{}", test_name));
    let _ = std::fs::remove_dir_all(&dir);
    let storage = Arc::new(Storage::new(dir.clone()));
    let event_bus = Arc::new(EventBus::new());
    let registry: Arc<dyn ProviderRegistry> =
        Arc::new(ProviderRegistryImpl::new(OJType::HOJ));
    let service = AuthService::new(registry, storage, Arc::clone(&event_bus));
    (service, event_bus, dir)
}

/// 在当前线程创建独立 tokio runtime，避免嵌套 runtime panic。
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("创建测试 runtime 失败");
    rt.block_on(fut)
}

fn session_path(dir: &std::path::Path) -> std::path::PathBuf {
    dir.join("sessions").join("HOJ.json")
}

fn sample_session(token: &str) -> Session {
    Session {
        user_id: "u1".into(),
        username: "tester".into(),
        token: token.into(),
        oj_type: "HOJ".into(),
    }
}

#[test]
fn save_and_restore_session_roundtrip() {
    let (service, _bus, dir) = make_service("roundtrip");
    service.save_session(&sample_session("token-a")).expect("保存会话失败");

    let restored = service.get_session().expect("应能恢复会话");
    assert_eq!(restored.token, "token-a");
    assert_eq!(restored.username, "tester");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn token_refreshed_event_rewrites_disk_session() {
    let (service, bus, dir) = make_service("refresh-rewrite");
    service.save_session(&sample_session("old-token")).expect("保存会话失败");

    bus.publish(&AppEvent::Auth(AuthEvent::TokenRefreshed {
        token: "new-token".into(),
    }));

    let raw = std::fs::read_to_string(session_path(&dir)).expect("读取会话文件失败");
    let persisted: Session = serde_json::from_str(&raw).expect("会话文件反序列化失败");
    assert_eq!(persisted.token, "new-token");
    // 非凭证字段保持不变
    assert_eq!(persisted.username, "tester");
    assert_eq!(persisted.user_id, "u1");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn token_refreshed_without_session_is_silently_ignored() {
    let (_service, bus, dir) = make_service("refresh-no-session");

    // 无磁盘会话时发布轮换事件：不应 panic，也不应创建新文件
    bus.publish(&AppEvent::Auth(AuthEvent::TokenRefreshed {
        token: "new-token".into(),
    }));

    assert!(!session_path(&dir).exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn token_refreshed_reaches_all_auth_subscribers() {
    let (_service, bus, dir) = make_service("refresh-fanout");

    let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter_clone = Arc::clone(&counter);
    bus.subscribe(
        EventCategory::Auth,
        Arc::new(move |event: &AppEvent| {
            if matches!(event, AppEvent::Auth(AuthEvent::TokenRefreshed { .. })) {
                counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        }),
    );

    bus.publish(&AppEvent::Auth(AuthEvent::TokenRefreshed {
        token: "t".into(),
    }));
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn logout_removes_disk_session() {
    let (service, _bus, dir) = make_service("logout-cleanup");
    service.save_session(&sample_session("token-a")).expect("保存会话失败");
    assert!(session_path(&dir).exists());

    // 未注册 AuthProvider：远端登出走非致命路径，本地会话仍应被清除
    block_on(service.logout()).expect("登出失败");

    assert!(!session_path(&dir).exists());
    assert!(service.get_session().is_none());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn corrupted_session_file_yields_none() {
    let (service, _bus, dir) = make_service("corrupted");

    std::fs::create_dir_all(dir.join("sessions")).expect("创建会话目录失败");
    std::fs::write(session_path(&dir), "{ not valid json").expect("写入损坏会话失败");

    assert!(service.get_session().is_none());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn clear_session_is_idempotent() {
    let (service, _bus, dir) = make_service("clear-idempotent");

    // 会话不存在时调用不报错
    service.clear_session(&OJType::HOJ);
    service.save_session(&sample_session("token-a")).expect("保存会话失败");
    service.clear_session(&OJType::HOJ);
    assert!(!session_path(&dir).exists());
    // 再次调用仍不报错
    service.clear_session(&OJType::HOJ);

    let _ = std::fs::remove_dir_all(&dir);
}
