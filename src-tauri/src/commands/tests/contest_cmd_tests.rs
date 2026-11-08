// `contest_cmd` 提取实现的测试。
//
// session_uid 此前只有「测试文件内复制的等价函数」锁定（mod_tests.rs 旧版），
// 现在直接调用生产函数；公告已读的无会话分支与 rank/公告分页钳制经 loopback
// 契约断言服务端实际收到的参数。

use super::*;
use crate::core::entity::session::Session;
use crate::core::provider::oj_id::OjId;
use crate::test_support::app_ctx::context_with_hoj;
use crate::test_support::loopback::{LoopbackServer, ScriptedResponse};

/// uid 解析：优先 HOJ 用户 UUID，旧版会话缺失 user_id 时回退 username（直调生产函数）。
#[tokio::test]
async fn session_uid_prefers_user_id_and_falls_back_to_username() {
    let server = LoopbackServer::start(vec![]).await;
    let (ctx, _dir) = context_with_hoj(server.url()).await;

    assert!(session_uid(&ctx).is_none(), "无会话应为 None");

    // 旧版会话：user_id 为空 → 回退 username
    ctx.session_repo
        .save(&Session::new("HOJ", "", "alice", "t1"))
        .unwrap();
    assert_eq!(session_uid(&ctx).as_deref(), Some("alice"));

    // 新版会话：优先 UUID
    ctx.session_repo
        .save(&Session::new("HOJ", "uuid-1", "alice", "t2"))
        .unwrap();
    assert_eq!(session_uid(&ctx).as_deref(), Some("uuid-1"));

    let _ = OjId::new("hoj");
    server.shutdown();
}

/// 无会话时公告已读的两个命令：get 返回空表、mark 静默跳过，都不报错。
#[tokio::test]
async fn announcement_read_commands_degrade_without_session() {
    let server = LoopbackServer::start(vec![]).await;
    let (ctx, _dir) = context_with_hoj(server.url()).await;

    assert_eq!(
        get_read_announcement_ids_impl(&ctx, "c1").unwrap(),
        Vec::<String>::new(),
        "无会话返回空列表（登录页也可能预渲染公告）"
    );
    mark_announcements_read_impl(&ctx, "c1", &["1".into()]).unwrap();
    assert!(
        !ctx.storage.exists("announcements_read"),
        "无会话时不得写入已读状态"
    );

    server.shutdown();
}

/// 公告分页默认值与钳制（server 实收断言，替代旧的复制函数测试）。
#[tokio::test]
async fn announcement_paging_defaults_and_clamping() {
    let server = LoopbackServer::start(vec![ScriptedResponse::json(
        r#"{"status":200,"msg":"success","data":{"records":[],"total":0,"size":50,"current":1,"pages":0}}"#,
    )])
    .await;
    let (ctx, _dir) = context_with_hoj(server.url()).await;

    list_contest_announcements_impl(&ctx, "1001".into(), Some(0), Some(-3))
        .await
        .expect("非法分页应钳制");

    let path = &server.requests()[0].path;
    assert!(path.contains("currentPage=1"), "钳制到 1: {}", path);
    assert!(path.contains("limit=1"), "负 limit 钳制到 1: {}", path);

    server.shutdown();
}

// ── 私有赛密码：命令入口的归一化契约 ──

/// 私有赛（auth=1）的比赛元信息响应（get-contest-info）。
const CONTEST_AUTH1: &str = r#"{"status":200,"msg":"success","data":{"id":1001,"author":"a","title":"私有赛","type":0,"description":"","status":0,"source":0,"auth":1,"now":null,"startTime":"2026-09-21T16:00:00.000+0000","endTime":"2026-09-29T16:00:00.000+0000","duration":100,"sealRank":null,"openPrint":null,"sealRankTime":null,"rankShowName":null,"openRank":false,"oiRankScoreType":"Recent","count":null,"gid":null,"allowEndSubmit":false}}"#;

/// 未报名私有赛的题目列表响应（真实消息，触发凭证进入流程）。
const PROBLEMS_UNREGISTERED: &str =
    r#"{"status":403,"msg":"请先到比赛首页输入比赛密码进行注册！","data":null}"#;

/// 权限查询：未报名。
const ACCESS_FALSE: &str = r#"{"status":200,"msg":"success","data":{"access":false}}"#;

/// 去空白后为空的比赛密码在命令入口归一化为 None：config.json 里的 Some("")
/// 不得以空密码发起注册（用户会看到「密码错误」而不是「需要比赛密码」的
/// 引导），loopback 不得收到任何 register-contest POST。
#[tokio::test]
async fn load_configured_contest_normalizes_blank_password_to_none() {
    let server = LoopbackServer::start(vec![
        ScriptedResponse::json(CONTEST_AUTH1), // get-contest-info（auth=1）
        ScriptedResponse::json(PROBLEMS_UNREGISTERED), // get-contest-problem（触发凭证进入）
        ScriptedResponse::json(ACCESS_FALSE),  // get-contest-access（未报名）
    ])
    .await;
    let (ctx, _dir) = crate::test_support::app_ctx::context_with_hoj_oj(
        server.url(),
        crate::core::entity::config::OjConfig {
            contest_ref: "1001".into(),
            contest_password: Some("   ".into()), // 空白密码（手工编辑常见）
            ..Default::default()
        },
    )
    .await;

    // 会话恢复：require_token 需要已登录（无会话会被前置断言拦下，测不到归一化）；
    // get_session 的恢复路径顺带把 token 回注 provider
    ctx.session_repo
        .save(&Session::new("HOJ", "uuid-1", "alice", "t1"))
        .unwrap();
    crate::commands::auth_cmd::get_session_impl(&ctx);

    let err = load_configured_contest_impl(&ctx)
        .await
        .expect_err("未报名且无有效密码应报引导错误");
    assert!(
        err.user_message().contains("比赛密码"),
        "空密码归一化后应报「缺凭证」引导，实际 {}",
        err.user_message()
    );

    // 关键断言：全程只有 GET——空密码若被透传，这里会出现 register-contest POST
    for req in server.requests() {
        assert_eq!(
            req.method, "GET",
            "空密码不得发起注册 POST（实际打到 {}）",
            req.path
        );
    }

    server.shutdown();
}
