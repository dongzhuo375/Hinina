use super::*;

use crate::core::error::AppError;
use crate::core::provider::oj_id::OjId;
use crate::core::provider::registry::{ProviderRegistry, ProviderSet};

/// 注册表的查询侧语义（能力视角 + ProviderNotFound 契约）。
///
/// 身份字符串化 + ProviderSet Option 字段后，「未注册」与「已注册但缺该能力」
/// 都必须落 `ProviderNotFound` —— 这是 Service 层错误上抛路径依赖的稳定契约。
#[test]
fn unregistered_current_returns_provider_not_found() {
    let registry = ProviderRegistryImpl::new(OjId::new("HOJ"));
    // 未注册任何 OJ：四种能力均 ProviderNotFound
    assert!(matches!(registry.current_auth(), Err(AppError::ProviderNotFound(_))));
    assert!(matches!(registry.current_contest(), Err(AppError::ProviderNotFound(_))));
    assert!(matches!(registry.current_problem(), Err(AppError::ProviderNotFound(_))));
    assert!(matches!(
        registry.current_submission(),
        Err(AppError::ProviderNotFound(_))
    ));
    assert!(registry.list_available().is_empty());
}

#[test]
fn missing_capability_returns_provider_not_found() {
    // 部分实现（只提供 contest 能力）是合法注册：缺失能力报 ProviderNotFound
    let registry = ProviderRegistryImpl::new(OjId::new("X"));
    registry.register(OjId::new("X"), ProviderSet::default());
    assert!(matches!(registry.current_auth(), Err(AppError::ProviderNotFound(_))));

    // 切换到未注册的 OJ：同样 ProviderNotFound
    registry.set_current(OjId::new("Y"));
    assert!(matches!(registry.current_auth(), Err(AppError::ProviderNotFound(_))));
}

#[test]
fn register_overwrites_and_list_available_reflects_registration() {
    // 覆盖注册（同 id 二次注册）与 list_available 语义
    let registry = ProviderRegistryImpl::new(OjId::new("HOJ"));
    registry.register(OjId::new("HOJ"), ProviderSet::default());
    registry.register(OjId::new("HOJ"), ProviderSet::default());
    registry.register(OjId::new("QDUOJ"), ProviderSet::default());

    let mut ids: Vec<String> = registry.list_available().into_iter().map(|id| id.to_string()).collect();
    ids.sort();
    assert_eq!(ids, vec!["HOJ".to_string(), "QDUOJ".to_string()]);
}

#[test]
fn current_id_and_set_current_roundtrip() {
    let registry = ProviderRegistryImpl::new(OjId::new("HOJ"));
    assert_eq!(registry.current_id().to_string(), "HOJ");

    registry.set_current(OjId::new("QDUOJ"));
    assert_eq!(registry.current_id().to_string(), "QDUOJ");

    // 身份规整：首尾空白被去除（配置手改常见）
    assert_eq!(OjId::new("  HOJ  ").to_string(), "HOJ");
}
