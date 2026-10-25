// `switch_oj` 提取实现（switch_oj_impl）的全真上下文测试（M2 工作包 A）。
//
// 此前该命令的编排（按需补注册 → 注册校验 → 注册表切换 + 三服务缓存清理 →
// 配置落盘 → OjSwitched 事件）完全没有测试：命令签名依赖 `tauri::State`，
// 旧测试只能复制等价逻辑。现在经 `*_impl` 提取 + loopback 上下文直接验证生产行为。

use crate::commands::oj_cmd::switch_oj_impl;
use crate::core::error::AppError;
use crate::core::event::core_event::CoreEvent;
use crate::test_support::app_ctx::context_with_hoj;
use crate::test_support::loopback::LoopbackServer;

/// 切到未配置/未启用的 OJ：显式报 ProviderNotFound，不静默回退。
#[tokio::test]
async fn switch_to_unregistered_oj_reports_provider_not_found() {
    let (ctx, _dir) = context_with_hoj("http://127.0.0.1:1").await;

    let err = switch_oj_impl(&ctx, "hydro").await.unwrap_err();
    match err {
        AppError::ProviderNotFound(msg) => {
            assert!(msg.contains("hydro"), "错误应点名目标 OJ: {}", msg)
        }
        other => panic!("应为 ProviderNotFound，实得 {:?}", other),
    }
    // 未切换：当前仍是配置的 hoj
    assert_eq!(ctx.provider_registry.current_id().as_str(), "HOJ");
}

/// 成功切换的完整链路：注册表切换、config.json 落盘 oj.active、发布 OjSwitched。
#[tokio::test]
async fn switch_oj_persists_config_and_publishes_event() {
    let server = LoopbackServer::start(vec![]).await;
    let (ctx, dir) = context_with_hoj(server.url()).await;

    // 事件订阅先于动作：OjSwitched 是事实通知，切换成功后必须可观察
    let mut events = ctx.event_bus.subscribe();

    switch_oj_impl(&ctx, "HOJ")
        .await
        .expect("切到已注册的 hoj 应成功");

    assert_eq!(ctx.provider_registry.current_id().as_str(), "HOJ");
    // 配置落盘：config.json 中 oj.active 已更新（而不是只改内存）
    let persisted = std::fs::read_to_string(dir.join("config.json")).unwrap();
    assert!(
        persisted.contains("\"active\"") && persisted.contains("HOJ"),
        "config.json 应持久化 oj.active=hoj: {}",
        persisted
    );
    // 事件到达（带超时兜底，防止用例挂死）。配置落盘本身也发布 ConfigChanged，
    // 故在有限窗口内跳过其它事实事件、只认 OjSwitched
    let mut got_switched = false;
    for _ in 0..4 {
        match tokio::time::timeout(std::time::Duration::from_secs(2), events.recv()).await {
            Ok(Ok(CoreEvent::OjSwitched { oj_id })) => {
                assert_eq!(oj_id, "HOJ");
                got_switched = true;
                break;
            }
            Ok(Ok(_)) => continue, // ConfigChanged 等其它事实事件
            other => panic!("应收到 OjSwitched 事件，实得 {:?}", other),
        }
    }
    assert!(got_switched, "有限事件窗口内未观察到 OjSwitched");

    server.shutdown();
}

/// 幂等性：重复切到同一 OJ 不报错（补注册短路，注册表已含目标）。
#[tokio::test]
async fn switch_oj_to_same_target_is_idempotent() {
    let server = LoopbackServer::start(vec![]).await;
    let (ctx, _dir) = context_with_hoj(server.url()).await;

    switch_oj_impl(&ctx, "HOJ").await.unwrap();
    switch_oj_impl(&ctx, "HOJ").await.unwrap();

    server.shutdown();
}
