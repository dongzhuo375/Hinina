// `contest_cmd` 提取实现的测试（M2 工作包 A）。
//
// session_uid 此前只有「测试文件内复制的等价函数」锁定（mod_tests.rs 旧版），
// 现在直接调用生产函数；公告已读的无会话分支与 rank/公告分页钳制经 loopback
// 契约断言服务端实际收到的参数。

use crate::commands::contest_cmd::{
    get_read_announcement_ids_impl, list_contest_announcements_impl, mark_announcements_read_impl,
    session_uid,
};
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
