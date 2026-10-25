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

use super::*;
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
    let problem_fixture = || {
        ScriptedResponse::json(
            r#"{"status":200,"msg":"success","data":{"problem":{"id":42,"title":"P42"},"languages":["C++"]}}"#,
        )
    };
    let submission_fixture = || {
        ScriptedResponse::json(
            r#"{"status":200,"msg":"success","data":{"submission":{"submitId":5,"status":2}}}"#,
        )
    };
    // 队列按真实请求顺序：预热 contest→problem→submission；切回后
    // contest→submission 重新拉取，problem 走磁盘缓存（键带 OJ 维度）不排队
    let hoj = LoopbackServer::start(vec![
        contest_list(),
        problem_fixture(),
        submission_fixture(),
        contest_list(),
        submission_fixture(),
        problem_fixture(),
    ])
    .await;
    let hydro = LoopbackServer::start(vec![]).await;
    let (ctx, dir) = context_with_two_ojs(hoj.url(), hydro.url()).await;
    let mut events = ctx.event_bus.subscribe();

    // 预热：三个服务的内存缓存各装一份（服务端各第 1 次被请求）
    let contests = ctx.contest.list_contests(60).await.expect("列表应成功");
    assert_eq!(contests.len(), 1);
    let problem = ctx
        .problem
        .open_problem("1001", "A", true)
        .await
        .expect("题面应成功");
    assert_eq!(problem.id, "42");
    let detail = ctx
        .submission
        .get_submission_detail("5")
        .await
        .expect("详情应成功");
    assert_eq!(detail.submit_id, "5", "status=2 为终态，详情进入内存缓存");
    assert_eq!(hoj.requests().len(), 3);

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

    // 缓存清理观察（评审 #7：三个服务的 on_oj_switched 必须都被调用）：
    // TTL（60s）内再次查询，若切换时未清内存缓存则直接命中；实测各自收到
    // 第 2 次请求 = contest / submission 内存缓存确实被清。
    // problem 的题面磁盘缓存键带 OJ 维度（设计如此，磁盘命中不发请求），
    // 其内存 statement_cache 的清理由 contest 之外的两次断言间接覆盖——
    // 此处用 detail 与 list 两个内存缓存做请求计数观察。
    let contests = ctx.contest.list_contests(60).await.expect("列表应成功");
    assert_eq!(contests.len(), 1);
    assert_eq!(
        hoj.requests().len(),
        4,
        "比赛列表必须重新拉取（预热 3 次 + 切回后复查 1 次；contest 缓存已清）"
    );
    let detail = ctx
        .submission
        .get_submission_detail("5")
        .await
        .expect("详情应成功");
    assert_eq!(detail.submit_id, "5");
    assert_eq!(
        hoj.requests().len(),
        5,
        "提交详情必须重新拉取（submission 用户域缓存已清，status=2 终态在预热时入缓存）"
    );

    // problem 内存 statement_cache 的清理断言（评审 #7 补严）：
    // 磁盘题面缓存键带 OJ 维度、切 OJ 时刻意保留（空间回收由异步任务做），
    // 因此先删除磁盘层排除其干扰——此后 open_problem 是否打网络**只**取决于
    // 内存 statement_cache 是否已被 on_oj_switched 清空。
    // problem 内存 statement_cache 的清理断言（评审 #7 补严）：
    // on_oj_switched 会**异步 spawn** 清理磁盘题面缓存（键带 OJ 维度，属空间
    // 回收）——轮询等清理落地；若超时仍在则显式删除兜底。此后 open_problem
    // 是否打网络**只**取决于内存 statement_cache 是否已被切换清空。
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while ctx.storage.exists("cache/problem_statement") && std::time::Instant::now() < deadline {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let _ = ctx.storage.remove_all("cache/problem_statement");
    let requests_before = hoj.requests().len();
    let problem = ctx
        .problem
        .open_problem("1001", "A", true)
        .await
        .expect("题面应成功");
    assert_eq!(problem.id, "42");
    assert_eq!(
        hoj.requests().len(),
        requests_before + 1,
        "磁盘层已删：能否打网络只取决于内存 statement_cache——必须已被切换清空"
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
