// `switch_oj` 提取实现（switch_oj_impl）的全真上下文测试（M2 工作包 A）。
//
// 此前该命令的编排（按需补注册 → 注册校验 → 注册表切换 + 三服务缓存清理 →
// 配置落盘 → OjSwitched 事件）完全没有测试：命令签名依赖 `tauri::State`，
// 旧测试只能复制等价逻辑。现在经 `*_impl` 提取 + loopback 上下文直接验证生产行为。
//
// 评审 P2 修订：切换测试使用**双 OJ 上下文**做真实切换（HOJ → Hydro → HOJ）。
// 单 OJ 上下文里「切到自己」无法证明注册表切换发生过，substring 断言也无法
// 证明配置持久化生效 —— 现以解析后的 `oj.active` 全值与「切回后缓存已清」的
// 请求计数观察为准。

use crate::commands::oj_cmd::switch_oj_impl;
use crate::core::error::AppError;
use crate::core::event::core_event::CoreEvent;
use crate::test_support::app_ctx::{context_with_hoj, context_with_two_ojs};
use crate::test_support::loopback::{LoopbackServer, ScriptedResponse};

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
    // 未切换：当前仍是配置的 HOJ
    assert_eq!(ctx.provider_registry.current_id().as_str(), "HOJ");
}

/// HOJ 比赛列表的成功响应（ContestVO 全字段 serde(default)，最小桩即可）。
fn contest_list() -> ScriptedResponse {
    ScriptedResponse::json(
        r#"{"status":200,"msg":"success","data":{"records":[{"id":1,"title":"C1"}],"total":1,"size":1,"current":1,"pages":1}}"#,
    )
}

/// 真实切换的完整链路（HOJ → Hydro → HOJ）：
/// 注册表当前值变化、config.json 的 `oj.active` 以解析值断言、两次 OjSwitched、
/// 切回后比赛列表缓存已清（第二次 list 重新打到服务端，而非命中切换前的内存缓存）。
#[tokio::test]
async fn real_switch_moves_registry_persists_config_and_clears_caches() {
    let hoj = LoopbackServer::start(vec![contest_list(), contest_list()]).await;
    let hydro = LoopbackServer::start(vec![]).await;
    let (ctx, dir) = context_with_two_ojs(hoj.url(), hydro.url()).await;
    let mut events = ctx.event_bus.subscribe();

    // 预热：HOJ 的比赛列表进入内存缓存（服务端第 1 次被请求）
    let contests = ctx.contest.list_contests(60).await.expect("列表应成功");
    assert_eq!(contests.len(), 1);
    assert_eq!(hoj.requests().len(), 1);

    // ── 切到 Hydro：注册表、持久化、事件 ──
    switch_oj_impl(&ctx, "Hydro")
        .await
        .expect("切到已注册的 Hydro 应成功");
    assert_eq!(ctx.provider_registry.current_id().as_str(), "Hydro");
    let persisted: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("config.json")).unwrap()).unwrap();
    assert_eq!(
        persisted["oj"]["active"], "Hydro",
        "config.json 应以解析值持久化 oj.active（substring 断言对原本就含 HOJ 的文件无效）"
    );

    // ── 切回 HOJ ──
    switch_oj_impl(&ctx, "HOJ").await.expect("切回 HOJ 应成功");
    assert_eq!(ctx.provider_registry.current_id().as_str(), "HOJ");
    let persisted: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("config.json")).unwrap()).unwrap();
    assert_eq!(persisted["oj"]["active"], "HOJ");

    // 缓存清理观察：TTL（60s）内再次 list，若切换时未清内存缓存则直接命中
    // （服务端仍只有 1 次请求）；实测收到第 2 次请求 = on_oj_switched 确实清了缓存
    let contests = ctx.contest.list_contests(60).await.expect("列表应成功");
    assert_eq!(contests.len(), 1);
    assert_eq!(
        hoj.requests().len(),
        2,
        "切回后必须重新拉取：切换时已清内存缓存"
    );

    // 事件：两次 OjSwitched（ConfigChanged 会交错，过滤后按序断言）
    let mut switched = Vec::new();
    for _ in 0..8 {
        if let Ok(Ok(CoreEvent::OjSwitched { oj_id })) =
            tokio::time::timeout(std::time::Duration::from_secs(2), events.recv()).await
        {
            switched.push(oj_id);
            if switched.len() == 2 {
                break;
            }
        }
    }
    assert_eq!(switched, vec!["Hydro".to_string(), "HOJ".to_string()]);

    hoj.shutdown();
    hydro.shutdown();
}

/// 重复切到同一目标：幂等成功（补注册短路，注册表已含目标）。
#[tokio::test]
async fn switch_oj_to_same_target_is_idempotent() {
    let hoj = LoopbackServer::start(vec![]).await;
    let hydro = LoopbackServer::start(vec![]).await;
    let (ctx, _dir) = context_with_two_ojs(hoj.url(), hydro.url()).await;

    switch_oj_impl(&ctx, "Hydro").await.unwrap();
    switch_oj_impl(&ctx, "Hydro").await.unwrap();
    assert_eq!(ctx.provider_registry.current_id().as_str(), "Hydro");

    hoj.shutdown();
    hydro.shutdown();
}
