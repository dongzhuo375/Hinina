// AppContext 的纯判定逻辑测试。
//
// 只测可从配置直接判定的部分（实例挑选）：`ensure_oj_registered` 需要完整的
// AppContext（http/事件总线/存储/注册表），构造代价远大于收益，其正确性由
// 「判定条件与 `init` 的注册条件同源」这条约束保证 —— 两条路径都走本文件的
// `enabled_instance`。

use super::*;
use crate::core::entity::config::OjInstance;
use crate::test_support::{Guarded, TempDir};

fn instance(id: &str, enabled: bool) -> OjInstance {
    OjInstance {
        id: id.to_string(),
        base_url: format!("https://{}.example.com", id.to_lowercase()),
        enabled,
        options: serde_json::Map::new(),
    }
}

fn config_with(instances: Vec<OjInstance>) -> AppConfig {
    let mut config = AppConfig::default();
    config.oj.instances = instances;
    config
}

#[test]
fn enabled_instance_finds_matching_enabled_entry() {
    let config = config_with(vec![instance("HOJ", true), instance("Hydro", true)]);
    assert_eq!(
        enabled_instance(&config, "Hydro").map(|i| i.id.as_str()),
        Some("Hydro")
    );
    assert_eq!(
        enabled_instance(&config, "HOJ").map(|i| i.id.as_str()),
        Some("HOJ")
    );
}

#[test]
fn enabled_instance_skips_disabled_entries() {
    // 禁用实例不会被注册：按需注册必须与 init 同判据，否则会绕开「禁用」开关
    let config = config_with(vec![instance("HOJ", true), instance("Hydro", false)]);
    assert!(enabled_instance(&config, "Hydro").is_none());
}

#[test]
fn enabled_instance_returns_none_for_unknown_or_mismatched_id() {
    let config = config_with(vec![instance("HOJ", true)]);
    // 未配置的 id：不能凭空激活（否则 switch_oj 成了绕过配置的后门）
    assert!(enabled_instance(&config, "Hydro").is_none());
    // 大小写敏感：id 是精确匹配的键（会话文件名同源），不做模糊归一
    assert!(enabled_instance(&config, "hoj").is_none());
    assert!(enabled_instance(&config, "").is_none());
}

#[test]
fn enabled_instance_handles_empty_instance_list() {
    let config = config_with(Vec::new());
    assert!(enabled_instance(&config, "HOJ").is_none());
}

#[test]
fn default_config_has_exactly_one_enabled_hoj_instance() {
    // 默认配置是「开箱即用 HOJ」：按需注册与启动注册都以它为唯一来源
    let config = AppConfig::default();
    assert_eq!(
        enabled_instance(&config, "HOJ").map(|i| i.id.as_str()),
        Some("HOJ")
    );
    assert_eq!(
        config.oj.instances.iter().filter(|i| i.enabled).count(),
        1,
        "默认只应有一个启用实例"
    );
}

// ── register_instance（init 与按需注册共用的实现）──
//
// 这条路径的回归价值：若「启动时注册」与「切换时补注册」各自实现，会漂移成
// 「重启后能用、切换时不能用」这类最难排查的不一致。故用真实注册表 + 真实工厂
// 锁定共享实现的行为。

fn test_registry() -> Arc<dyn ProviderRegistry> {
    Arc::new(crate::infra::provider_registry_impl::ProviderRegistryImpl::new(OjId::new("HOJ")))
}

fn test_deps(tag: &str) -> Guarded<crate::adapter::AdapterDeps> {
    let dir = TempDir::named(&format!("hinina-test-context-{}", tag));
    let storage = Arc::new(crate::infra::storage::Storage::new(dir.to_path_buf()));
    let deps = crate::adapter::test_adapter_deps(storage);
    Guarded::new(deps, dir)
}

#[test]
fn register_instance_registers_known_oj() {
    let registry = test_registry();
    let deps = test_deps("register-known");
    let hoj = instance("HOJ", true);

    assert!(register_instance(registry.as_ref(), &deps, &hoj));
    let available = registry.list_available();
    assert!(
        available.contains(&OjId::new("HOJ")),
        "注册后应出现在可用清单: {:?}",
        available
    );
}

#[test]
fn register_instance_skips_unknown_oj_without_panicking() {
    // 配置里写了没有适配器的 OJ：必须 warn + false，且不影响其它 OJ 的注册
    let registry = test_registry();
    let deps = test_deps("register-unknown");

    assert!(!register_instance(
        registry.as_ref(),
        &deps,
        &instance("NotAnOj", true)
    ));
    assert!(
        registry.list_available().is_empty(),
        "未知 OJ 不应注册任何东西"
    );
}

#[test]
fn register_instance_is_idempotent() {
    // 幂等：重复注册同一个 OJ 不应 panic，也不应产生重复项
    let registry = test_registry();
    let deps = test_deps("register-idempotent");
    let hoj = instance("HOJ", true);

    assert!(register_instance(registry.as_ref(), &deps, &hoj));
    assert!(register_instance(registry.as_ref(), &deps, &hoj));
    assert_eq!(registry.list_available().len(), 1);
}

#[test]
fn register_instance_matches_factory_by_exact_id() {
    // 工厂按 id 精确匹配（大小写敏感）：`hoj` 不是 `HOJ`，应跳过而不是误注册
    let registry = test_registry();
    let deps = test_deps("register-case");

    assert!(!register_instance(
        registry.as_ref(),
        &deps,
        &instance("hoj", true)
    ));
    assert!(registry.list_available().is_empty());
}
