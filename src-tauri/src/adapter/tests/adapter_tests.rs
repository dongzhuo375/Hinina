use super::*;

use std::sync::Arc;

use crate::core::event::event_bus::EventBus;
use crate::infra::http::HttpClient;
use crate::infra::storage::Storage;
use crate::test_support::TempDir;

/// 工厂清单的编译期防线（取代闭集枚举的穷尽检查）：
/// id 唯一、全部可构建、至少提供一个能力。接入新 OJ 后本测试自动覆盖新工厂。
///
/// 刻意**不断言四能力齐备**：`ProviderSet` 的 Option 字段正是「新 Adapter
/// 可先只实现部分接口」的扩展路径（开发手册明文），全能力断言会让部分实现的
/// OJ 一接入就红灯 —— 那是把扩展路径焊死。此处只锁定「可构建且非空集」
/// （全 None 的 ProviderSet 属注册 bug）。
#[test]
fn factory_ids_unique_and_buildable() {
    let list = factories();
    assert!(!list.is_empty(), "工厂清单不能为空");

    // id 唯一（重复 id 会让注册表互相覆盖、会话文件名撞车）
    let ids: Vec<&'static str> = list.iter().map(|f| f.id()).collect();
    let mut deduped = ids.clone();
    deduped.sort();
    deduped.dedup();
    assert_eq!(ids.len(), deduped.len(), "工厂 id 必须唯一: {:?}", ids);

    // 全部可构建（依赖只来自 infra —— AdapterDeps 不含任何 Service）
    // 且至少提供一个能力（全 None 是注册 bug）
    let dir = TempDir::named("hinina-test-adapter-factory");
    let deps = AdapterDeps {
        http_client: Arc::new(HttpClient::with_timeout(std::time::Duration::from_secs(5)).expect("HttpClient 构造失败")),
        event_bus: Arc::new(EventBus::new()),
        storage: Arc::new(Storage::new(dir.to_path_buf())),
    };

    for factory in &list {
        let set = factory.build(&deps, "https://example.com");
        let capability_count = set.auth.is_some() as usize
            + set.contest.is_some() as usize
            + set.problem.is_some() as usize
            + set.submission.is_some() as usize;
        assert!(capability_count > 0, "{} 至少需提供一个能力（全空 ProviderSet 是注册 bug）", factory.id());
    }

}

/// HOJ 工厂身份契约：id 决定会话文件名，须与历史枚举 Debug 输出一致。
#[test]
fn hoj_factory_id_matches_session_file_contract() {
    assert_eq!(crate::adapter::hoj::HOJAdapter::ID, "HOJ");
    assert_eq!(
        crate::core::provider::oj_id::OjId::new(crate::adapter::hoj::HOJAdapter::ID).session_file(),
        "HOJ.json"
    );
}

/// HOJ 工厂四能力齐备的专属锁定。
///
/// 通用断言（`factory_ids_unique_and_buildable`）为保住「先实现部分接口」的
/// 扩展路径只查「至少一个能力」；HOJ 是当前唯一全功能内建 OJ，其四项能力
/// 齐备在此显式锁定 —— 若工厂漏装某个能力，不该等到运行期 ProviderNotFound。
#[test]
fn hoj_factory_provides_all_four_capabilities() {
    let dir = TempDir::named("hinina-test-hoj-factory-capabilities");
    let deps = AdapterDeps {
        http_client: Arc::new(HttpClient::with_timeout(std::time::Duration::from_secs(5)).expect("HttpClient 构造失败")),
        event_bus: Arc::new(EventBus::new()),
        storage: Arc::new(Storage::new(dir.to_path_buf())),
    };

    let set = crate::adapter::hoj::FACTORY.build(&deps, "https://example.com");
    assert!(set.auth.is_some(), "HOJ 缺 Auth 能力");
    assert!(set.contest.is_some(), "HOJ 缺 Contest 能力");
    assert!(set.problem.is_some(), "HOJ 缺 Problem 能力");
    assert!(set.submission.is_some(), "HOJ 缺 Submission 能力");

}
