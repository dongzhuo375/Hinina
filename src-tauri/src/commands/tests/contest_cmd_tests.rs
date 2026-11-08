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

// ── 私有赛密码：clear_access_attempts 的三条命令接线（换账号的关键入口）──

/// 注册被拒响应（包络 400 + 真实消息，非 401/403 不判会话失效）。
const REGISTER_REJECTED: &str = r#"{"status":400,"msg":"比赛密码错误，请重新输入！","data":null}"#;

/// 登录成功响应：token 在响应头（HOJ 约定），身份在信封 data。
const LOGIN_OK: &str = r#"{"status":200,"msg":"success","data":{"uid":"7","username":"alice"}}"#;

/// 登出响应（HOJAdapter 忽略响应内容，以清 token 为主）。
const LOGOUT_OK: &str = r#"{"status":200,"msg":"success","data":null}"#;

/// 供接线测试装配「指向 loopback 的 HOJ + 私有赛配置」的会话种子。
async fn wired_context(server: &LoopbackServer) -> (AppContext, crate::test_support::TempDir) {
    let (ctx, dir) = crate::test_support::app_ctx::context_with_hoj_oj(
        server.url(),
        crate::core::entity::config::OjConfig {
            contest_ref: "1001".into(),
            contest_password: Some("bad".into()),
            ..Default::default()
        },
    )
    .await;
    // 会话恢复：get_session 顺带把 token 回注 provider（require_token 需要）
    ctx.session_repo
        .save(&Session::new("HOJ", "uuid-1", "alice", "t1"))
        .unwrap();
    crate::commands::auth_cmd::get_session_impl(&ctx);
    (ctx, dir)
}

fn access_query_count(server: &LoopbackServer) -> usize {
    server
        .requests()
        .iter()
        .filter(|r| r.path.contains("get-contest-access"))
        .count()
}

/// logout 接线：进入失败（密码错误 → Rejected 记忆）→ 登出 → 再加载必须
/// **重新发起进入**。观察点是错误形态：记忆残留时 lookup Failure 提前返回、
/// 复用「密码错误」消息（Contest 变体）；已清则重新触达 provider，匿名会话
/// 被 require_token 前置断言拦下（Auth「请先登录」）。
#[tokio::test]
async fn logout_clears_access_attempts_so_next_load_retouches_provider() {
    let server = LoopbackServer::start(vec![
        ScriptedResponse::json(CONTEST_AUTH1),
        ScriptedResponse::json(PROBLEMS_UNREGISTERED),
        ScriptedResponse::json(ACCESS_FALSE),
        ScriptedResponse::json(REGISTER_REJECTED),
        ScriptedResponse::json(LOGOUT_OK),
        ScriptedResponse::json(PROBLEMS_UNREGISTERED),
    ])
    .await;
    let (ctx, _dir) = wired_context(&server).await;

    let err1 = load_configured_contest_impl(&ctx)
        .await
        .expect_err("首次密码错误应报错");
    assert!(
        err1.user_message().contains("比赛密码错误"),
        "实得 {}",
        err1.user_message()
    );

    crate::commands::auth_cmd::logout_impl(&ctx).await.unwrap();

    let err2 = load_configured_contest_impl(&ctx)
        .await
        .expect_err("第二次加载仍应失败（匿名会话）");
    assert!(
        matches!(err2, AppError::Auth(ref m) if m.contains("请先登录")),
        "记忆已清：重新发起的进入被 require_token 拦下（Auth 变体触发会话守卫），实际 {:?}",
        err2
    );

    server.shutdown();
}

/// login 接线：进入失败（密码错误 → Rejected 记忆）→ 登录成功 → 同密码再加载
/// 必须重新发起进入。观察点是 access 查询再次出现——记忆残留时 lookup Failure
/// 提前返回、不会发 access GET（变异：删 login_impl 的清空调用，本用例红）。
#[tokio::test]
async fn login_clears_access_attempts_so_retry_retouches_provider() {
    let server = LoopbackServer::start(vec![
        ScriptedResponse::json(CONTEST_AUTH1),
        ScriptedResponse::json(PROBLEMS_UNREGISTERED),
        ScriptedResponse::json(ACCESS_FALSE),
        ScriptedResponse::json(REGISTER_REJECTED),
        ScriptedResponse::with(
            200,
            vec![("Authorization".into(), "jwt-2".into())],
            LOGIN_OK,
        ),
        ScriptedResponse::json(PROBLEMS_UNREGISTERED),
        ScriptedResponse::json(ACCESS_FALSE),
        ScriptedResponse::json(REGISTER_REJECTED),
    ])
    .await;
    let (ctx, _dir) = wired_context(&server).await;

    let err1 = load_configured_contest_impl(&ctx)
        .await
        .expect_err("首次密码错误应报错");
    assert!(err1.user_message().contains("比赛密码错误"));

    let user = crate::commands::auth_cmd::login_impl(&ctx, "alice", "pw")
        .await
        .expect("登录应成功");
    assert_eq!(user.username, "alice");

    let err2 = load_configured_contest_impl(&ctx)
        .await
        .expect_err("第二次加载仍应报错（脚本仍拒）");
    assert!(
        err2.user_message().contains("比赛密码错误"),
        "重新发起后仍是服务端真实拒绝（非复用消息），实得 {}",
        err2.user_message()
    );
    assert_eq!(
        access_query_count(&server),
        2,
        "login 清空记忆：第二次加载重新发起进入（access 查询再次出现）"
    );

    server.shutdown();
}

/// reset_client 接线：进入失败（密码错误 → Rejected 记忆）→ 重置 → 再加载必须
/// 重新发起进入。reset 不清会话（token 仍在），access 查询照常走到——
/// 记忆残留时 lookup Failure 提前返回、access GET 不会出现（变异即红）。
#[tokio::test]
async fn reset_client_clears_access_attempts() {
    let server = LoopbackServer::start(vec![
        ScriptedResponse::json(CONTEST_AUTH1),
        ScriptedResponse::json(PROBLEMS_UNREGISTERED),
        ScriptedResponse::json(ACCESS_FALSE),
        ScriptedResponse::json(REGISTER_REJECTED),
        ScriptedResponse::json(CONTEST_AUTH1), // reset 清了元信息内存缓存：load#2 重新拉
        ScriptedResponse::json(PROBLEMS_UNREGISTERED),
        ScriptedResponse::json(ACCESS_FALSE),
        ScriptedResponse::json(REGISTER_REJECTED),
    ])
    .await;
    let (ctx, _dir) = wired_context(&server).await;

    let err1 = load_configured_contest_impl(&ctx)
        .await
        .expect_err("首次密码错误应报错");
    assert!(err1.user_message().contains("比赛密码错误"));

    crate::commands::maintenance_cmd::reset_client_impl(&ctx).unwrap();

    let err2 = load_configured_contest_impl(&ctx)
        .await
        .expect_err("第二次加载仍应报错（脚本仍拒）");
    assert!(err2.user_message().contains("比赛密码错误"));
    assert_eq!(
        access_query_count(&server),
        2,
        "reset_client 清空记忆：第二次加载重新发起进入（access 查询再次出现）"
    );

    server.shutdown();
}

/// 合法密码带首尾空格时**保留原值**：归一化只判空、不裁剪——静默裁剪会
/// 改写用户的真实密码（注册请求体必须携带原值，变异：改回 trim 即红）。
/// 拒绝形态与 adapter 契约测试一致（HTTP 200 + 体内 status=400），并断言
/// **错误变体**为 Contest——若只断言响应体摘录，HTTP 400 的 Network 摘录
/// 也能蒙混过关（假绿）。
#[tokio::test]
async fn load_configured_contest_preserves_password_with_spaces() {
    let server = LoopbackServer::start(vec![
        ScriptedResponse::json(CONTEST_AUTH1),
        ScriptedResponse::json(PROBLEMS_UNREGISTERED),
        ScriptedResponse::json(ACCESS_FALSE),
        ScriptedResponse::json(REGISTER_REJECTED),
    ])
    .await;
    let (ctx, _dir) = crate::test_support::app_ctx::context_with_hoj_oj(
        server.url(),
        crate::core::entity::config::OjConfig {
            contest_ref: "1001".into(),
            contest_password: Some("  pw  ".into()), // 带首尾空格的合法密码
            ..Default::default()
        },
    )
    .await;
    ctx.session_repo
        .save(&Session::new("HOJ", "uuid-1", "alice", "t1"))
        .unwrap();
    crate::commands::auth_cmd::get_session_impl(&ctx);

    let err = load_configured_contest_impl(&ctx)
        .await
        .expect_err("脚本仍拒，应报错");
    assert!(
        matches!(err, AppError::Contest(ref m) if m.contains("比赛密码错误")),
        "拒绝必须走信封业务路径（Contest 变体），而不是 HTTP 400 的 Network 摘录，实际 {:?}",
        err
    );

    // 注册请求体必须携带原值（含空格）——静默裁剪会让用户以错误的密码注册
    let reqs = server.requests();
    let register = reqs
        .iter()
        .find(|r| r.path.contains("register-contest"))
        .expect("应发起注册请求");
    let body: serde_json::Value = serde_json::from_str(&register.body).unwrap();
    assert_eq!(
        body["password"], "  pw  ",
        "密码必须保留原值（只判空、不裁剪）"
    );

    server.shutdown();
}

/// login 接线：**登录失败也清进入记忆**。provider.login 被服务端接受后适配器
/// 内存已是新 token——随后会话落盘失败让整体返回 Err（`?` 会提前返回）；
/// 旧账号的结论记忆必须失效，否则新 token 复用旧账号的 Rejected/Success。
/// 观察点：失败登录后再加载，access 查询必须再次出现（记忆未清则 lookup
/// 命中 Failure 提前返回、不发 access GET）。
#[tokio::test]
async fn failed_login_still_clears_access_attempts() {
    let server = LoopbackServer::start(vec![
        ScriptedResponse::json(CONTEST_AUTH1),
        ScriptedResponse::json(PROBLEMS_UNREGISTERED),
        ScriptedResponse::json(ACCESS_FALSE),
        ScriptedResponse::json(REGISTER_REJECTED),
        // 登录失败形态：200 但缺 Authorization 头 → adapter 报 Auth
        ScriptedResponse::json(LOGIN_OK),
        ScriptedResponse::json(PROBLEMS_UNREGISTERED),
        ScriptedResponse::json(ACCESS_FALSE),
        ScriptedResponse::json(REGISTER_REJECTED),
    ])
    .await;
    let (ctx, _dir) = wired_context(&server).await;

    let err1 = load_configured_contest_impl(&ctx)
        .await
        .expect_err("首次密码错误应报错");
    assert!(err1.user_message().contains("比赛密码错误"));

    let login_err = crate::commands::auth_cmd::login_impl(&ctx, "alice", "pw")
        .await
        .expect_err("缺 token 头的登录应失败");
    assert!(
        matches!(login_err, AppError::Auth(_)),
        "登录失败应为 Auth，实际 {:?}",
        login_err
    );

    let err2 = load_configured_contest_impl(&ctx)
        .await
        .expect_err("第二次加载仍应报错（脚本仍拒）");
    assert!(
        err2.user_message().contains("比赛密码错误"),
        "重新发起后应是服务端真实拒绝（非复用旧账号消息），实得 {}",
        err2.user_message()
    );
    assert_eq!(
        access_query_count(&server),
        2,
        "登录失败也必须清空记忆：第二次加载重新发起进入（access 查询再次出现）"
    );

    server.shutdown();
}
