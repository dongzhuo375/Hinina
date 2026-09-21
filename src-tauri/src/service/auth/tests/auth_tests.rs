use super::*;

use std::sync::Arc;

use crate::core::error::AppError;
use crate::core::event::event_category::EventCategory;
use crate::core::provider::auth::AuthProvider;
use crate::core::provider::oj_id::OjId;
use crate::core::provider::registry::ProviderSet;
use crate::infra::provider_registry_impl::ProviderRegistryImpl;
use crate::test_support::TempDir;

/// 构造基于独立临时目录的 AuthService 及其依赖。
fn make_service(test_name: &str) -> (AuthService, Arc<EventBus>, TempDir) {
    let (service, event_bus, dir, _registry) = make_service_with_auth(test_name, None);
    (service, event_bus, dir)
}

/// 同 `make_service`，但可注入 AuthProvider（会话校验路径测试用）。
fn make_service_with_auth(
    test_name: &str,
    provider: Option<Arc<dyn AuthProvider>>,
) -> (
    AuthService,
    Arc<EventBus>,
    TempDir,
    Arc<dyn ProviderRegistry>,
) {
    let dir = TempDir::named(&format!("hinina-test-auth-{}", test_name));
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let event_bus = Arc::new(EventBus::new());
    let registry: Arc<dyn ProviderRegistry> =
        Arc::new(ProviderRegistryImpl::new(OjId::new("HOJ")));
    if let Some(p) = provider {
        registry.register(
            OjId::new("HOJ"),
            ProviderSet {
                auth: Some(p),
                ..Default::default()
            },
        );
    }
    let service = AuthService::new(Arc::clone(&registry), storage, Arc::clone(&event_bus));
    (service, event_bus, dir, registry)
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
        oj_id: "HOJ".into(),
    }
}

#[test]
fn save_and_restore_session_roundtrip() {
    let (service, _bus, _dir) = make_service("roundtrip");
    service.save_session(&sample_session("token-a")).expect("保存会话失败");

    let restored = service.get_session().expect("应能恢复会话");
    assert_eq!(restored.token, "token-a");
    assert_eq!(restored.username, "tester");

}

#[test]
fn token_refreshed_event_rewrites_disk_session() {
    let (service, bus, dir) = make_service("refresh-rewrite");
    service.save_session(&sample_session("old-token")).expect("保存会话失败");

    bus.publish(&AppEvent::Auth(AuthEvent::TokenRefreshed {
        token: "new-token".into(),
    }));

    let raw = std::fs::read_to_string(session_path(dir.path())).expect("读取会话文件失败");
    let persisted: Session = serde_json::from_str(&raw).expect("会话文件反序列化失败");
    assert_eq!(persisted.token, "new-token");
    // 非凭证字段保持不变
    assert_eq!(persisted.username, "tester");
    assert_eq!(persisted.user_id, "u1");

}

#[test]
fn token_refreshed_without_session_is_silently_ignored() {
    let (_service, bus, dir) = make_service("refresh-no-session");

    // 无磁盘会话时发布轮换事件：不应 panic，也不应创建新文件
    bus.publish(&AppEvent::Auth(AuthEvent::TokenRefreshed {
        token: "new-token".into(),
    }));

    assert!(!session_path(dir.path()).exists());
}

#[test]
fn token_refreshed_reaches_all_auth_subscribers() {
    let (_service, bus, _dir) = make_service("refresh-fanout");

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

}

#[test]
fn logout_removes_disk_session() {
    let (service, _bus, dir) = make_service("logout-cleanup");
    service.save_session(&sample_session("token-a")).expect("保存会话失败");
    assert!(session_path(dir.path()).exists());

    // 未注册 AuthProvider：远端登出走非致命路径，本地会话仍应被清除
    block_on(service.logout()).expect("登出失败");

    assert!(!session_path(dir.path()).exists());
    assert!(service.get_session().is_none());

}

#[test]
fn corrupted_session_file_yields_none() {
    let (service, _bus, dir) = make_service("corrupted");

    std::fs::create_dir_all(dir.join("sessions")).expect("创建会话目录失败");
    std::fs::write(session_path(dir.path()), "{ not valid json").expect("写入损坏会话失败");

    assert!(service.get_session().is_none());

}

#[test]
fn clear_session_is_idempotent() {
    let (service, _bus, dir) = make_service("clear-idempotent");

    // 会话不存在时调用不报错
    service.clear_session(&OjId::new("HOJ"));
    service.save_session(&sample_session("token-a")).expect("保存会话失败");
    service.clear_session(&OjId::new("HOJ"));
    assert!(!session_path(dir.path()).exists());
    // 再次调用仍不报错
    service.clear_session(&OjId::new("HOJ"));

}

// ── 身份数据化（OjId）的持久化契约 ──

#[test]
fn oj_id_session_file_matches_legacy_enum_debug_output() {
    // 契约：内建 OJ 的 id 必须与历史枚举变体的 Debug 输出一致，
    // 否则升级后既有 sessions/{OJ}.json 全部失联（静默丢会话）
    assert_eq!(crate::adapter::hoj::HOJAdapter::ID, "HOJ");
    assert_eq!(OjId::new(crate::adapter::hoj::HOJAdapter::ID).session_file(), "HOJ.json");
    assert_eq!(OjId::new("HOJ").to_string(), "HOJ");
}

#[test]
fn session_deserializes_legacy_oj_type_key() {
    // 旧版会话文件以 `oj_type` 为键名存储 OJ 身份；字段更名 `oj_id` 后
    // 须经 serde alias 兼容读取，否则升级即丢会话
    let raw = r#"{"user_id":"u1","username":"team01","token":"tk","oj_type":"HOJ"}"#;
    let session: Session = serde_json::from_str(raw).expect("旧键名会话应可反序列化");
    assert_eq!(session.oj_id, "HOJ");

    // 新键名正常往返
    let json = serde_json::to_string(&sample_session("tk")).expect("序列化失败");
    let back: Session = serde_json::from_str(&json).expect("新键名会话应可反序列化");
    assert_eq!(back.oj_id, "HOJ");
}

// ── 会话校验（三态）──

/// Stub Provider 的预设校验结果
#[derive(Clone, Copy)]
enum StubOutcome {
    /// 服务端确认有效
    Valid,
    /// 服务端明确判定失效
    Invalid,
    /// 请求失败（网络异常）
    NetworkErr,
}

/// 测试用 AuthProvider：以固定预设结果响应校验，并记录被回注的 token。
struct StubAuthProvider {
    outcome: StubOutcome,
    restored: Arc<std::sync::Mutex<Option<String>>>,
}

#[async_trait::async_trait]
impl AuthProvider for StubAuthProvider {
    async fn login(&self, username: &str, _password: &str) -> AppResult<User> {
        Ok(User {
            id: "u1".into(),
            username: username.into(),
            token: "stub-token".into(),
        })
    }

    async fn logout(&self) -> AppResult<()> {
        Ok(())
    }

    async fn validate_session(&self) -> AppResult<bool> {
        match self.outcome {
            StubOutcome::Valid => Ok(true),
            StubOutcome::Invalid => Ok(false),
            StubOutcome::NetworkErr => Err(AppError::Network("stub: 网络异常".into())),
        }
    }

    fn restore_token(&self, token: &str) {
        if let Ok(mut slot) = self.restored.lock() {
            *slot = Some(token.to_string());
        }
    }
}

/// 构造带 Stub AuthProvider 的服务与 token 回注记录槽。
fn make_stub_service(
    test_name: &str,
    outcome: StubOutcome,
) -> (
    AuthService,
    Arc<EventBus>,
    TempDir,
    Arc<std::sync::Mutex<Option<String>>>,
) {
    let restored = Arc::new(std::sync::Mutex::new(None));
    let provider = Arc::new(StubAuthProvider {
        outcome,
        restored: Arc::clone(&restored),
    });
    let (service, bus, dir, _registry) = make_service_with_auth(test_name, Some(provider));
    (service, bus, dir, restored)
}

#[test]
fn validate_session_without_local_session_is_invalid() {
    let (service, _bus, _dir, _reg) = make_service_with_auth("validate-no-session", None);

    assert_eq!(
        block_on(service.validate_session()),
        SessionValidity::Invalid,
        "本地无会话应判定为 Invalid"
    );

}

#[test]
fn validate_session_without_provider_is_unknown_and_keeps_session() {
    let (service, _bus, dir, _reg) = make_service_with_auth("validate-no-provider", None);
    service.save_session(&sample_session("token-a")).expect("保存会话失败");

    assert_eq!(
        block_on(service.validate_session()),
        SessionValidity::Unknown,
        "无 Provider 时无法判定，应为 Unknown"
    );
    assert!(session_path(dir.path()).exists(), "无法判定时不应清除本地会话");

}

#[test]
fn validate_session_valid_restores_token_and_keeps_session() {
    let (service, _bus, dir, restored) = make_stub_service("validate-valid", StubOutcome::Valid);
    service.save_session(&sample_session("token-a")).expect("保存会话失败");

    assert_eq!(block_on(service.validate_session()), SessionValidity::Valid);
    assert!(session_path(dir.path()).exists(), "有效会话不应被清除");
    // 校验前必须把磁盘 token 回注 Provider，否则应用重启后的校验请求必然 401
    assert_eq!(
        restored.lock().expect("锁中毒").as_deref(),
        Some("token-a"),
        "校验前应回注磁盘会话的 token"
    );

}

#[test]
fn validate_session_invalid_clears_session_and_publishes_event() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let (service, bus, dir, _restored) =
        make_stub_service("validate-invalid", StubOutcome::Invalid);
    service.save_session(&sample_session("token-a")).expect("保存会话失败");

    let expired = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&expired);
    bus.subscribe(
        EventCategory::Auth,
        Arc::new(move |event: &AppEvent| {
            if matches!(event, AppEvent::Auth(AuthEvent::SessionExpired)) {
                flag.store(true, Ordering::SeqCst);
            }
        }),
    );

    assert_eq!(block_on(service.validate_session()), SessionValidity::Invalid);
    assert!(!session_path(dir.path()).exists(), "服务端判定失效应清除磁盘会话");
    assert!(expired.load(Ordering::SeqCst), "应发布 SessionExpired 事件");
    assert!(service.get_session().is_none());

}

#[test]
fn validate_session_network_error_is_unknown_and_keeps_session() {
    let (service, _bus, dir, _restored) =
        make_stub_service("validate-network-err", StubOutcome::NetworkErr);
    service.save_session(&sample_session("token-a")).expect("保存会话失败");

    assert_eq!(
        block_on(service.validate_session()),
        SessionValidity::Unknown,
        "网络异常不等于会话失效"
    );
    assert!(session_path(dir.path()).exists(), "网络异常时应保留本地会话");

}

#[test]
fn session_validity_serializes_as_snake_case() {
    // 序列化形态属于跨端契约：前端按 'valid' / 'invalid' / 'unknown' 分支处理
    assert_eq!(
        serde_json::to_string(&SessionValidity::Valid).expect("序列化失败"),
        "\"valid\""
    );
    assert_eq!(
        serde_json::to_string(&SessionValidity::Invalid).expect("序列化失败"),
        "\"invalid\""
    );
    assert_eq!(
        serde_json::to_string(&SessionValidity::Unknown).expect("序列化失败"),
        "\"unknown\""
    );
}
