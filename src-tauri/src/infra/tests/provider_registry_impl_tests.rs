use super::*;

use crate::core::error::AppError;
use crate::core::provider::contest::ContestProvider;
use crate::core::provider::oj_id::OjId;
use crate::core::provider::registry::{ProviderRegistry, ProviderSet};

/// 快照测试用的最小 contest provider（本文件只关心 id 与 provider 的配对）。
struct SnapshotStubProvider;

#[async_trait::async_trait]
impl ContestProvider for SnapshotStubProvider {
    async fn list_contests(&self) -> AppResult<Vec<crate::core::entity::contest::Contest>> {
        unreachable!("快照测试不调用能力方法")
    }

    async fn get_contest(
        &self,
        _contest_id: &str,
    ) -> AppResult<crate::core::entity::contest::Contest> {
        unreachable!("快照测试不调用能力方法")
    }

    async fn list_contest_problems(
        &self,
        _contest_id: &str,
    ) -> AppResult<Vec<crate::core::entity::contest::ContestProblem>> {
        unreachable!("快照测试不调用能力方法")
    }

    async fn get_contest_rank(
        &self,
        _contest_id: &str,
        _query: &crate::core::entity::rank::RankQuery,
    ) -> AppResult<crate::core::entity::rank::ContestRankPage> {
        unreachable!("快照测试不调用能力方法")
    }

    async fn list_announcements(
        &self,
        _contest_id: &str,
        _current_page: i64,
        _limit: i64,
    ) -> AppResult<crate::core::entity::announcement::AnnouncementPage> {
        unreachable!("快照测试不调用能力方法")
    }
}

/// 注册表的查询侧语义（能力视角 + ProviderNotFound 契约）。
///
/// 身份字符串化 + ProviderSet Option 字段后，「未注册」与「已注册但缺该能力」
/// 都必须落 `ProviderNotFound` —— 这是 Service 层错误上抛路径依赖的稳定契约。
#[test]
fn unregistered_current_returns_provider_not_found() {
    let registry = ProviderRegistryImpl::new(OjId::new("HOJ"), Arc::default());
    // 未注册任何 OJ：四种能力均 ProviderNotFound
    assert!(matches!(
        registry.current_auth(),
        Err(AppError::ProviderNotFound(_))
    ));
    assert!(matches!(
        registry.current_contest(),
        Err(AppError::ProviderNotFound(_))
    ));
    assert!(matches!(
        registry.current_problem(),
        Err(AppError::ProviderNotFound(_))
    ));
    assert!(matches!(
        registry.current_submission(),
        Err(AppError::ProviderNotFound(_))
    ));
    assert!(registry.list_available().is_empty());
}

#[test]
fn missing_capability_returns_provider_not_found() {
    // 部分实现（只提供 contest 能力）是合法注册：缺失能力报 ProviderNotFound
    let registry = ProviderRegistryImpl::new(OjId::new("X"), Arc::default());
    registry.register(OjId::new("X"), ProviderSet::default());
    assert!(matches!(
        registry.current_auth(),
        Err(AppError::ProviderNotFound(_))
    ));

    // 切换到未注册的 OJ：同样 ProviderNotFound
    registry.set_current(OjId::new("Y"));
    assert!(matches!(
        registry.current_auth(),
        Err(AppError::ProviderNotFound(_))
    ));
}

#[test]
fn register_overwrites_and_list_available_reflects_registration() {
    // 覆盖注册（同 id 二次注册）与 list_available 语义
    let registry = ProviderRegistryImpl::new(OjId::new("HOJ"), Arc::default());
    registry.register(OjId::new("HOJ"), ProviderSet::default());
    registry.register(OjId::new("HOJ"), ProviderSet::default());
    registry.register(OjId::new("QDUOJ"), ProviderSet::default());

    let mut ids: Vec<String> = registry
        .list_available()
        .into_iter()
        .map(|id| id.to_string())
        .collect();
    ids.sort();
    assert_eq!(ids, vec!["HOJ".to_string(), "QDUOJ".to_string()]);
}

#[test]
fn current_id_and_set_current_roundtrip() {
    let registry = ProviderRegistryImpl::new(OjId::new("HOJ"), Arc::default());
    assert_eq!(registry.current_id().to_string(), "HOJ");

    registry.set_current(OjId::new("QDUOJ"));
    assert_eq!(registry.current_id().to_string(), "QDUOJ");

    // 身份规整：首尾空白被去除（配置手改常见）
    assert_eq!(OjId::new("  HOJ  ").to_string(), "HOJ");
}

#[test]
fn current_contest_snapshot_is_id_consistent_across_switch() {
    // 同源快照契约：provider 与归属 id 必须来自**同一个**捕获的 OJ——
    // 两次独立读取在切换竞态下会拿到 A 的 provider 配 B 的 id
    //（进入校验的记忆键因此错配）。快照按捕获 id 反查 provider，天然自洽。
    // 双 OJ 各挂**独立 stub 实例**，快照返回的 provider 经 Arc 数据指针与
    // 注册实例比对——上一版两个 OJ 共用同一 stub 类型且只断言 id，
    // 区分不了「provider 与 id 配错」的错配
    let registry = ProviderRegistryImpl::new(OjId::new("HOJ"), Arc::default());
    let hoj_stub = Arc::new(SnapshotStubProvider);
    let hydro_stub = Arc::new(SnapshotStubProvider);
    registry.register(
        OjId::new("HOJ"),
        ProviderSet {
            contest: Some(Arc::clone(&hoj_stub) as Arc<dyn ContestProvider>),
            ..Default::default()
        },
    );
    registry.register(
        OjId::new("HYDRO"),
        ProviderSet {
            contest: Some(Arc::clone(&hydro_stub) as Arc<dyn ContestProvider>),
            ..Default::default()
        },
    );

    let (provider, id) = registry.current_contest_snapshot().expect("已注册应成功");
    assert_eq!(id.to_string(), "HOJ");
    // Arc 数据指针同源：dyn 胖指针与具体指针的数据地址可比
    assert_eq!(
        Arc::as_ptr(&provider) as *const u8,
        Arc::as_ptr(&hoj_stub) as *const u8,
        "快照返回的 provider 必须是 HOJ 注册的那个实例"
    );

    registry.set_current(OjId::new("HYDRO"));
    let (provider, id) = registry.current_contest_snapshot().expect("已注册应成功");
    assert_eq!(id.to_string(), "HYDRO", "切换后快照跟随新 OJ");
    assert_eq!(
        Arc::as_ptr(&provider) as *const u8,
        Arc::as_ptr(&hydro_stub) as *const u8,
        "切换后快照返回的必须是 HYDRO 注册的实例"
    );
    assert_ne!(
        Arc::as_ptr(&provider) as *const u8,
        Arc::as_ptr(&hoj_stub) as *const u8,
        "不同 OJ 的注册实例互不同源"
    );

    // 未注册的当前 OJ：快照同样 ProviderNotFound（与 current_contest 契约一致）
    registry.set_current(OjId::new("MISSING"));
    assert!(matches!(
        registry.current_contest_snapshot(),
        Err(AppError::ProviderNotFound(_))
    ));
}

/// 并发切换压力测试：写线程持续翻转 set_current，主线程并行取快照并断言
/// **id 与 provider 的配对**（provider 身份经 Arc 数据指针识别）。
///
/// 这是「同源」契约的真正锁：朴素实现（分别调 `current_contest()` 与
/// `current_id()`）在翻转线程命中间隙时会产出「A 的 provider 配 B 的 id」
/// ——指针与 id 的配对断言变红。当前实现捕获 id 一次后按 id 反查 provider，
/// 任一时刻的配对都自洽，因此本用例在正确实现下**确定性通过**（与调度无关）。
#[test]
fn concurrent_switches_never_mix_provider_and_id() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let registry = std::sync::Arc::new(ProviderRegistryImpl::new(
        OjId::new("HOJ"),
        std::sync::Arc::default(),
    ));
    let hoj_stub = std::sync::Arc::new(SnapshotStubProvider);
    let hydro_stub = std::sync::Arc::new(SnapshotStubProvider);
    registry.register(
        OjId::new("HOJ"),
        ProviderSet {
            contest: Some(std::sync::Arc::clone(&hoj_stub) as std::sync::Arc<dyn ContestProvider>),
            ..Default::default()
        },
    );
    registry.register(
        OjId::new("HYDRO"),
        ProviderSet {
            contest: Some(std::sync::Arc::clone(&hydro_stub) as std::sync::Arc<dyn ContestProvider>),
            ..Default::default()
        },
    );

    let hoj_ptr = std::sync::Arc::as_ptr(&hoj_stub) as *const u8;
    let hydro_ptr = std::sync::Arc::as_ptr(&hydro_stub) as *const u8;
    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let stop_for_thread = std::sync::Arc::clone(&stop);
    let flipper_registry = std::sync::Arc::clone(&registry);
    let flipper = std::thread::spawn(move || {
        let mut toggle = false;
        while !stop_for_thread.load(Ordering::Relaxed) {
            toggle = !toggle;
            flipper_registry.set_current(OjId::new(if toggle { "HYDRO" } else { "HOJ" }));
            std::hint::spin_loop();
        }
    });

    let mut mismatches = 0;
    for _ in 0..2_000 {
        let (provider, id) = registry.current_contest_snapshot().expect("两 OJ 均已注册");
        let ptr = std::sync::Arc::as_ptr(&provider) as *const u8;
        let expected = match id.to_string().as_str() {
            "HOJ" => hoj_ptr,
            "HYDRO" => hydro_ptr,
            other => panic!("意外的 OJ: {other}"),
        };
        if ptr != expected {
            mismatches += 1;
        }
    }
    stop.store(true, Ordering::Relaxed);
    flipper.join().expect("翻转线程不应 panic");

    assert_eq!(
        mismatches, 0,
        "快照的 provider 与 id 必须始终同源（朴素两读实现在翻转下会配错）"
    );
}
