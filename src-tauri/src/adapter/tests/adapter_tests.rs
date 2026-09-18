use super::*;

use std::sync::Arc;

use crate::core::event::event_bus::EventBus;
use crate::infra::http::HttpClient;
use crate::infra::storage::Storage;

/// 工厂清单的编译期防线（取代闭集枚举的穷尽检查）：
/// id 唯一、全部可构建、四能力齐备。接入新 OJ 后本测试自动覆盖新工厂。
#[test]
fn factory_ids_unique_and_fully_buildable() {
    let list = factories();
    assert!(!list.is_empty(), "工厂清单不能为空");

    // id 唯一（重复 id 会让注册表互相覆盖、会话文件名撞车）
    let ids: Vec<&'static str> = list.iter().map(|f| f.id()).collect();
    let mut deduped = ids.clone();
    deduped.sort();
    deduped.dedup();
    assert_eq!(ids.len(), deduped.len(), "工厂 id 必须唯一: {:?}", ids);

    // 全部可构建（依赖只来自 infra —— AdapterDeps 不含任何 Service）
    let dir = std::env::temp_dir().join("hinina-test-adapter-factory");
    let _ = std::fs::remove_dir_all(&dir);
    let deps = AdapterDeps {
        http_client: Arc::new(HttpClient::with_timeout(std::time::Duration::from_secs(5)).expect("HttpClient 构造失败")),
        event_bus: Arc::new(EventBus::new()),
        storage: Arc::new(Storage::new(dir.clone())),
    };

    for factory in &list {
        let set = factory.build(&deps, "https://example.com");
        assert!(set.auth.is_some(), "{} 缺 Auth 能力", factory.id());
        assert!(set.contest.is_some(), "{} 缺 Contest 能力", factory.id());
        assert!(set.problem.is_some(), "{} 缺 Problem 能力", factory.id());
        assert!(set.submission.is_some(), "{} 缺 Submission 能力", factory.id());
    }

    let _ = std::fs::remove_dir_all(&dir);
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
