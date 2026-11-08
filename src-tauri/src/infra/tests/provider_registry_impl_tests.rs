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
    //（进入校验的记忆键因此错配）。快照按捕获 id 反查 provider，天然自洽
    let registry = ProviderRegistryImpl::new(OjId::new("HOJ"), Arc::default());
    registry.register(
        OjId::new("HOJ"),
        ProviderSet {
            contest: Some(Arc::new(SnapshotStubProvider)),
            ..Default::default()
        },
    );
    registry.register(
        OjId::new("HYDRO"),
        ProviderSet {
            contest: Some(Arc::new(SnapshotStubProvider)),
            ..Default::default()
        },
    );

    let (provider, id) = registry.current_contest_snapshot().expect("已注册应成功");
    assert_eq!(id.to_string(), "HOJ");
    let _ = provider; // 同一 stub 类型无法按值区分，id 一致即同源（provider 按 id 反查）

    registry.set_current(OjId::new("HYDRO"));
    let (_, id) = registry.current_contest_snapshot().expect("已注册应成功");
    assert_eq!(id.to_string(), "HYDRO", "切换后快照跟随新 OJ");

    // 未注册的当前 OJ：快照同样 ProviderNotFound（与 current_contest 契约一致）
    registry.set_current(OjId::new("MISSING"));
    assert!(matches!(
        registry.current_contest_snapshot(),
        Err(AppError::ProviderNotFound(_))
    ));
}
