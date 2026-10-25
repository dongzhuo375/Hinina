// `logout` / `get_session` 提取实现的全真上下文测试（M2 工作包 A）。
//
// 重点锁定：登出后的本地会话清除必须无条件发生（远端登出失败也不影响），
// 以及 Session → User 的 VO 映射（前端按这三个字段读取）。

use super::*;
use crate::core::entity::session::Session;
use crate::core::event::core_event::CoreEvent;
use crate::core::provider::oj_id::OjId;
use crate::test_support::app_ctx::context_with_hoj;
use crate::test_support::loopback::LoopbackServer;

fn seed_session(ctx: &crate::core::context::AppContext) {
    ctx.session_repo
        .save(&Session::new("HOJ", "uuid-1", "alice", "jwt-token"))
        .unwrap();
    // get_session 会顺带把 token 回注 Provider；logout 的远端调用也因此会发出
    ctx.auth.get_session().expect("种子会话应可加载");
}

/// 远端登出失败（500）时：本地会话仍被清除、命令仍返回 Ok、LoggedOut 仍发布。
#[tokio::test]
async fn logout_clears_local_session_even_when_remote_logout_fails() {
    // 对列空：远端登出 GET /api/logout 会拿到「队列耗尽 500」——正好充当远端失败
    let server = LoopbackServer::start(vec![]).await;
    let (ctx, _dir) = context_with_hoj(server.url()).await;
    seed_session(&ctx);
    let mut events = ctx.event_bus.subscribe();

    logout_impl(&ctx).await.expect("远端失败时登出仍应成功");

    assert!(
        ctx.session_repo.load(&OjId::new("HOJ")).unwrap().is_none(),
        "本地会话必须被清除"
    );
    match tokio::time::timeout(std::time::Duration::from_secs(2), events.recv()).await {
        Ok(Ok(CoreEvent::LoggedOut { oj_id })) => assert_eq!(oj_id, "HOJ"),
        other => panic!("应收到 LoggedOut 事件，实得 {:?}", other),
    }
    // 远端确实被调用过（携带种子 token 的 Authorization 头）
    let reqs = server.requests();
    let logout_call = reqs.iter().find(|r| r.path.starts_with("/api/logout"));
    assert!(logout_call.is_some(), "远端登出应被尝试: {:?}", reqs);
    assert_eq!(
        logout_call.unwrap().header("authorization"),
        Some("jwt-token")
    );

    server.shutdown();
}

/// 无会话登出：无远端调用（HOJ 无 token 时跳过远端），本地幂等成功。
#[tokio::test]
async fn logout_without_session_is_local_only() {
    let server = LoopbackServer::start(vec![]).await;
    let (ctx, _dir) = context_with_hoj(server.url()).await;

    logout_impl(&ctx).await.unwrap();

    assert!(server.requests().is_empty(), "无 token 不应发远端请求");
    server.shutdown();
}

/// get_session 的 VO 映射：Session 三字段 → User（id 取 user_id，token 透传）。
#[tokio::test]
async fn get_session_maps_session_to_user_vo() {
    let server = LoopbackServer::start(vec![]).await;
    let (ctx, _dir) = context_with_hoj(server.url()).await;

    assert!(get_session_impl(&ctx).is_none(), "无会话时应为 None");

    ctx.session_repo
        .save(&Session::new("HOJ", "uuid-1", "alice", "jwt-token"))
        .unwrap();
    let user = get_session_impl(&ctx).expect("有种子的会话应映射为 User");
    assert_eq!(user.id, "uuid-1");
    assert_eq!(user.username, "alice");
    assert_eq!(user.token, "jwt-token");

    server.shutdown();
}
