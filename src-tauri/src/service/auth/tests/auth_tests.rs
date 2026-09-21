use super::*;

use std::sync::Arc;

use crate::core::error::AppError;
use crate::core::event::core_event::CoreEvent;
use crate::core::event::core_event_bus::CoreEventBus;
use crate::core::provider::auth::AuthProvider;
use crate::core::provider::oj_id::OjId;
use crate::core::provider::registry::ProviderSet;
use crate::core::repository::session_repo::SessionRepository;
use crate::infra::fs_session_repo::FsSessionRepository;
use crate::infra::provider_registry_impl::ProviderRegistryImpl;
use crate::infra::storage::Storage;
use crate::test_support::TempDir;

/// 测试服务的依赖束：服务 + 事件总线 + 会话仓库 + 临时目录守卫 + 注册表。
///
/// 抽成别名是为了让返回类型可读（clippy 也会对超长元组类型报警）。
type AuthServiceFixture = (
    AuthService,
    Arc<CoreEventBus>,
    Arc<dyn SessionRepository>,
    TempDir,
    Arc<dyn ProviderRegistry>,
);

/// 同上，但末位换成「token 回注记录槽」（Stub Provider 场景用）。
type StubAuthFixture = (
    AuthService,
    Arc<CoreEventBus>,
    Arc<dyn SessionRepository>,
    TempDir,
    Arc<std::sync::Mutex<Option<String>>>,
);

/// 构造基于独立临时目录的 AuthService 及其依赖。
///
/// 会话仓库（`Arc<dyn SessionRepository>`）一并交出：会话落盘现在由仓库承担，
/// 测试直接用它写盘/读盘，比经由 Service 的私有方法更贴近真实调用链。
fn make_service(
    test_name: &str,
) -> (
    AuthService,
    Arc<CoreEventBus>,
    Arc<dyn SessionRepository>,
    TempDir,
) {
    let (service, event_bus, repo, dir, _registry) = make_service_with_auth(test_name, None);
    (service, event_bus, repo, dir)
}

/// 同 `make_service`，但可注入 AuthProvider（会话校验路径测试用）。
fn make_service_with_auth(
    test_name: &str,
    provider: Option<Arc<dyn AuthProvider>>,
) -> AuthServiceFixture {
    let dir = TempDir::named(&format!("hinina-test-auth-{}", test_name));
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let event_bus = Arc::new(CoreEventBus::new());
    let session_repo: Arc<dyn SessionRepository> =
        Arc::new(FsSessionRepository::new(Arc::clone(&storage)));
    let registry: Arc<dyn ProviderRegistry> = Arc::new(ProviderRegistryImpl::new(OjId::new("HOJ")));
    if let Some(p) = provider {
        registry.register(
            OjId::new("HOJ"),
            ProviderSet {
                auth: Some(p),
                ..Default::default()
            },
        );
    }
    let service = AuthService::new(
        Arc::clone(&registry),
        Arc::clone(&session_repo),
        Arc::clone(&event_bus),
    );
    (service, event_bus, session_repo, dir, registry)
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
    Session::new("HOJ", "u1", "tester", token)
}

#[test]
fn save_and_restore_session_roundtrip() {
    let (service, _bus, repo, _dir) = make_service("roundtrip");
    repo.save(&sample_session("token-a")).expect("保存会话失败");

    let restored = service.get_session().expect("应能恢复会话");
    assert_eq!(restored.token, "token-a");
    assert_eq!(restored.username, "tester");
}

/// 登录成功后会话**确已落盘**，且 `LoggedIn` 只带标识。
///
/// 落盘与事件是两件事，必须分别断言：旧实现把落盘交给事件的同步订阅者，
/// 于是「登录成功」与「会话已保存」的正确性绑在了投递时序上。
#[test]
fn login_persists_session_then_publishes_logged_in_without_token() {
    let restored = Arc::new(std::sync::Mutex::new(None));
    let provider = Arc::new(StubAuthProvider {
        outcome: StubOutcome::Valid,
        restored: Arc::clone(&restored),
    });
    let (service, bus, _repo, dir, _registry) =
        make_service_with_auth("login-persist", Some(provider));
    let mut rx = bus.subscribe();

    let user = block_on(service.login("tester", "pw")).expect("登录应成功");
    assert_eq!(user.token, "stub-token");

    // ① 会话已落盘（登录返回即保证）
    let raw = std::fs::read_to_string(session_path(dir.path())).expect("会话文件应已写出");
    let persisted: Session = serde_json::from_str(&raw).expect("会话文件应可反序列化");
    assert_eq!(persisted.token, "stub-token");
    assert_eq!(persisted.oj_id, "HOJ");

    // ② 事件是脱敏事实通知：只带 OJ 与用户标识，不含凭证
    let event = rx.try_recv().expect("登录成功应发布 LoggedIn");
    assert_eq!(
        event,
        CoreEvent::LoggedIn {
            oj_id: "HOJ".into(),
            user_id: "u1".into(),
        }
    );
    assert!(
        !format!("{event:?}").contains("stub-token"),
        "凭证绝不能进入事件流"
    );
}

/// 会话落盘失败 → 登录整体失败且**不发布** `LoggedIn`：
/// 会话没落盘就宣告登录成功，会让重启后的客户端拿着不存在的会话工作。
#[test]
fn login_does_not_publish_when_session_persist_fails() {
    let restored = Arc::new(std::sync::Mutex::new(None));
    let provider = Arc::new(StubAuthProvider {
        outcome: StubOutcome::Valid,
        restored: Arc::clone(&restored),
    });
    let (service, bus, _repo, dir, _registry) =
        make_service_with_auth("login-persist-fail", Some(provider));
    let mut rx = bus.subscribe();

    // 把 sessions 占位成文件，使 create_dir / 写入必然失败（确定性，不依赖权限）
    std::fs::write(dir.join("sessions"), "not a directory").expect("占位失败");

    assert!(block_on(service.login("tester", "pw")).is_err());
    assert!(
        rx.try_recv().is_err(),
        "落盘失败不得发布 LoggedIn（否则重启后会话不存在）"
    );
}

#[test]
fn logout_removes_disk_session_then_publishes_logged_out() {
    let (service, bus, repo, dir) = make_service("logout-cleanup");
    repo.save(&sample_session("token-a")).expect("保存会话失败");
    assert!(session_path(dir.path()).exists());
    let mut rx = bus.subscribe();

    // 未注册 AuthProvider：远端登出走非致命路径，本地会话仍应被清除
    block_on(service.logout()).expect("登出失败");

    assert!(!session_path(dir.path()).exists());
    assert!(service.get_session().is_none());
    assert_eq!(
        rx.try_recv().expect("登出应发布 LoggedOut"),
        CoreEvent::LoggedOut {
            oj_id: "HOJ".into()
        }
    );
}

#[test]
fn corrupted_session_file_yields_none() {
    let (service, _bus, _repo, dir) = make_service("corrupted");

    std::fs::create_dir_all(dir.join("sessions")).expect("创建会话目录失败");
    std::fs::write(session_path(dir.path()), "{ not valid json").expect("写入损坏会话失败");

    // 仓库层如实报错，应用层降级为「无会话」（并 warn）——IPC 契约不变
    assert!(service.get_session().is_none());
}

#[test]
fn session_removal_is_idempotent() {
    let (_service, _bus, repo, dir) = make_service("clear-idempotent");

    // 会话不存在时调用不报错
    repo.remove(&OjId::new("HOJ"))
        .expect("删除不存在的会话不应报错");
    repo.save(&sample_session("token-a")).expect("保存会话失败");
    repo.remove(&OjId::new("HOJ")).expect("删除会话失败");
    assert!(!session_path(dir.path()).exists());
    // 再次调用仍不报错
    repo.remove(&OjId::new("HOJ")).expect("重复删除不应报错");
}

// ── 身份数据化（OjId）的持久化契约 ──

#[test]
fn oj_id_session_file_matches_legacy_enum_debug_output() {
    // 契约：内建 OJ 的 id 必须与历史枚举变体的 Debug 输出一致，
    // 否则升级后既有 sessions/{OJ}.json 全部失联（静默丢会话）
    assert_eq!(crate::adapter::hoj::HOJAdapter::ID, "HOJ");
    assert_eq!(
        OjId::new(crate::adapter::hoj::HOJAdapter::ID).session_file(),
        "HOJ.json"
    );
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
fn make_stub_service(test_name: &str, outcome: StubOutcome) -> StubAuthFixture {
    let restored = Arc::new(std::sync::Mutex::new(None));
    let provider = Arc::new(StubAuthProvider {
        outcome,
        restored: Arc::clone(&restored),
    });
    let (service, bus, repo, dir, _registry) = make_service_with_auth(test_name, Some(provider));
    (service, bus, repo, dir, restored)
}

#[test]
fn validate_session_without_local_session_is_invalid() {
    let (service, _bus, _repo, _dir, _reg) = make_service_with_auth("validate-no-session", None);

    assert_eq!(
        block_on(service.validate_session()),
        SessionValidity::Invalid,
        "本地无会话应判定为 Invalid"
    );
}

#[test]
fn validate_session_without_provider_is_unknown_and_keeps_session() {
    let (service, _bus, repo, dir, _reg) = make_service_with_auth("validate-no-provider", None);
    repo.save(&sample_session("token-a")).expect("保存会话失败");

    assert_eq!(
        block_on(service.validate_session()),
        SessionValidity::Unknown,
        "无 Provider 时无法判定，应为 Unknown"
    );
    assert!(
        session_path(dir.path()).exists(),
        "无法判定时不应清除本地会话"
    );
}

#[test]
fn validate_session_valid_restores_token_and_keeps_session() {
    let (service, _bus, repo, dir, restored) =
        make_stub_service("validate-valid", StubOutcome::Valid);
    repo.save(&sample_session("token-a")).expect("保存会话失败");

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
    let (service, bus, repo, dir, _restored) =
        make_stub_service("validate-invalid", StubOutcome::Invalid);
    repo.save(&sample_session("token-a")).expect("保存会话失败");
    let mut rx = bus.subscribe();

    assert_eq!(
        block_on(service.validate_session()),
        SessionValidity::Invalid
    );
    assert!(
        !session_path(dir.path()).exists(),
        "服务端判定失效应清除磁盘会话"
    );
    assert_eq!(
        rx.try_recv().expect("应发布 SessionExpired"),
        CoreEvent::SessionExpired {
            oj_id: "HOJ".into()
        }
    );
    assert!(service.get_session().is_none());
}

#[test]
fn validate_session_network_error_is_unknown_and_keeps_session() {
    let (service, _bus, repo, dir, _restored) =
        make_stub_service("validate-network-err", StubOutcome::NetworkErr);
    repo.save(&sample_session("token-a")).expect("保存会话失败");

    assert_eq!(
        block_on(service.validate_session()),
        SessionValidity::Unknown,
        "网络异常不等于会话失效"
    );
    assert!(
        session_path(dir.path()).exists(),
        "网络异常时应保留本地会话"
    );
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
