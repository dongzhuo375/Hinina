// AppContext 的纯判定逻辑测试。
//
// 只测可从配置直接判定的部分（实例挑选）：`ensure_oj_registered` 需要完整的
// AppContext（http/事件总线/存储/注册表），构造代价远大于收益，其正确性由
// 「判定条件与 `init` 的注册条件同源」这条约束保证 —— 两条路径都走本文件的
// `enabled_instance`。

use super::*;
use crate::core::entity::config::OjInstance;

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
    assert_eq!(enabled_instance(&config, "HOJ").map(|i| i.id.as_str()), Some("HOJ"));
    assert_eq!(
        config.oj.instances.iter().filter(|i| i.enabled).count(),
        1,
        "默认只应有一个启用实例"
    );
}
