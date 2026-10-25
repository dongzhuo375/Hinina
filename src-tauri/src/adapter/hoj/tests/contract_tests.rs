// HOJ 适配器的本地 scripted HTTP 契约测试（M2 工作包 B）。
//
// 与 mod_tests.rs 的 DTO/fixture 测试互补：这里打**真实网络栈**（reqwest →
// loopback server），锁定线上协议行为——登录凭证与 token 提取、裸 Authorization
// 头形态、token 轮换的显式落盘与事件、提交体字段与前置登录断言、POST 不重试。

use std::sync::Arc;

use crate::adapter::hoj::HOJAdapter;
use crate::core::event::core_event::CoreEvent;
use crate::core::event::core_event_bus::CoreEventBus;
use crate::core::provider::auth::AuthProvider;
use crate::core::provider::oj_id::OjId;
use crate::core::provider::submission::SubmissionProvider;
use crate::core::repository::session_repo::SessionRepository;
use crate::infra::fs_session_repo::FsSessionRepository;
use crate::infra::http::HttpClient;
use crate::infra::storage::Storage;
use crate::test_support::loopback::{LoopbackServer, ScriptedResponse};
use crate::test_support::TempDir;

struct Fixture {
    adapter: Arc<HOJAdapter>,
    session_repo: Arc<dyn SessionRepository>,
    event_bus: Arc<CoreEventBus>,
    _dir: TempDir,
}

/// 构造指向脚本 server 的真实适配器（真实 HttpClient/事件总线/会话仓库）。
fn fixture(base_url: &str) -> Fixture {
    let dir = TempDir::unique("hinina-hoj-contract");
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let session_repo: Arc<dyn SessionRepository> =
        Arc::new(FsSessionRepository::new(Arc::clone(&storage)));
    let event_bus = Arc::new(CoreEventBus::new());
    let http = Arc::new(HttpClient::with_timeout(std::time::Duration::from_secs(5)).unwrap());
    let adapter = Arc::new(HOJAdapter::new(
        http,
        base_url.to_string(),
        OjId::new("HOJ"),
        Arc::clone(&event_bus),
        Arc::clone(&session_repo),
    ));
    Fixture {
        adapter,
        session_repo,
        event_bus,
        _dir: dir,
    }
}

/// 登录成功响应：token 在响应头（HOJ 约定），身份在信封 data。
fn login_ok(token: &str) -> ScriptedResponse {
    ScriptedResponse::with(
        200,
        vec![("Authorization".into(), token.into())],
        r#"{"status":200,"msg":"success","data":{"uid":"7","username":"alice"}}"#,
    )
}

/// 提交列表成功响应（空页即可；JudgeVO 全字段 serde(default)）。
fn page_ok() -> ScriptedResponse {
    ScriptedResponse::json(
        r#"{"status":200,"msg":"success","data":{"records":[],"total":0,"size":20,"current":1,"pages":0}}"#,
    )
}

/// 登录：POST /api/login 携带明文凭证（服务端自行 MD5），token 取自响应头。
#[tokio::test]
async fn login_posts_credentials_and_reads_token_from_header() {
    let server = LoopbackServer::start(vec![login_ok("jwt-abc")]).await;
    let fx = fixture(server.url());

    let user = fx.adapter.login("alice", "pw").await.expect("登录应成功");

    assert_eq!(user.id, "7");
    assert_eq!(user.username, "alice");
    assert_eq!(user.token, "jwt-abc");

    let req = &server.requests()[0];
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, "/api/login");
    let body: serde_json::Value = serde_json::from_str(&req.body).unwrap();
    assert_eq!(body["username"], "alice");
    assert_eq!(body["password"], "pw", "密码必须明文发送（服务端自行 MD5）");

    server.shutdown();
}

/// 响应头缺失 token：判定为认证失败（而不是把无凭证会话当成功）。
#[tokio::test]
async fn login_without_token_header_fails_as_auth() {
    let server = LoopbackServer::start(vec![ScriptedResponse::json(
        r#"{"status":200,"msg":"success","data":{"uid":"7","username":"alice"}}"#,
    )])
    .await;
    let fx = fixture(server.url());

    let err = fx.adapter.login("alice", "pw").await.unwrap_err();
    assert!(
        matches!(err, crate::core::error::AppError::Auth(ref m) if m.contains("token")),
        "缺 token 头应为 Auth 错误，实得 {:?}",
        err
    );

    server.shutdown();
}

/// 认证请求带**裸** JWT（无 Bearer 前缀，HOJ 私有约定）。
#[tokio::test]
async fn authed_request_sends_bare_authorization_header() {
    let server = LoopbackServer::start(vec![login_ok("jwt-1"), page_ok()]).await;
    let fx = fixture(server.url());

    fx.adapter.login("alice", "pw").await.unwrap();
    let query = crate::core::entity::submission::SubmissionQuery {
        contest_id: "1001".into(),
        current_page: 1,
        limit: 20,
        only_mine: true,
        problem_display_id: None,
        status: None,
    };
    fx.adapter
        .list_contest_submissions(&query)
        .await
        .expect("列表应成功");

    let list_req = &server.requests()[1];
    assert_eq!(
        list_req.header("authorization"),
        Some("jwt-1"),
        "HOJ 的 JWT 直接放在 Authorization 头，无 Bearer 前缀"
    );

    server.shutdown();
}

/// token 轮换：响应头 `Refresh-Token: true` + 新 Authorization →
/// 后续请求用新 token、新 token 显式落盘、发布 TokenRotated（只带 OJ 标识）。
#[tokio::test]
async fn token_rotation_updates_memory_disk_and_next_request() {
    let rotated = ScriptedResponse::with(
        200,
        vec![
            ("Refresh-Token".into(), "true".into()),
            ("Authorization".into(), "jwt-2".into()),
        ],
        page_ok().body,
    );
    let server = LoopbackServer::start(vec![login_ok("jwt-1"), rotated, page_ok()]).await;
    let fx = fixture(server.url());
    let mut events = fx.event_bus.subscribe();

    fx.adapter.login("alice", "pw").await.unwrap();
    // 登录会话落盘由 AuthService 负责；适配器层轮换写盘要求会话已存在，
    // 这里手工补一份（模拟 AuthService 登录后的持久化时机）
    fx.session_repo
        .save(&crate::core::entity::session::Session::new(
            "HOJ", "7", "alice", "jwt-1",
        ))
        .unwrap();

    let query = crate::core::entity::submission::SubmissionQuery {
        contest_id: "1001".into(),
        current_page: 1,
        limit: 20,
        only_mine: true,
        problem_display_id: None,
        status: None,
    };
    fx.adapter.list_contest_submissions(&query).await.unwrap();
    fx.adapter.list_contest_submissions(&query).await.unwrap();

    let reqs = server.requests();
    assert_eq!(
        reqs[2].header("authorization"),
        Some("jwt-2"),
        "轮换后应使用新 token"
    );
    // 显式落盘：磁盘会话的 token 已换新
    let persisted = fx
        .session_repo
        .load(&OjId::new("HOJ"))
        .unwrap()
        .expect("会话应存在");
    assert_eq!(persisted.token, "jwt-2");
    // 事件：TokenRotated 只带 OJ 标识，token 绝不进入事件流
    match tokio::time::timeout(std::time::Duration::from_secs(2), events.recv()).await {
        Ok(Ok(CoreEvent::TokenRotated { oj_id })) => assert_eq!(oj_id, "HOJ"),
        other => panic!("应收到 TokenRotated，实得 {:?}", other),
    }

    server.shutdown();
}

/// 提交：未登录直接 Auth 报错（不发请求）；登录后 POST 体以比赛展示题号为 pid。
#[tokio::test]
async fn submit_requires_token_and_uses_display_pid() {
    let server = LoopbackServer::start(vec![
        login_ok("jwt-1"),
        ScriptedResponse::json(r#"{"status":200,"msg":"success","data":{"submitId":123}}"#),
    ])
    .await;
    let fx = fixture(server.url());

    // 未登录：前置断言，不打网络
    let err = fx
        .adapter
        .submit("1001", "p-42", "A", "C++", "int main(){}")
        .await
        .unwrap_err();
    assert!(matches!(err, crate::core::error::AppError::Auth(_)));
    assert!(server.requests().is_empty(), "前置断言不应发请求");

    fx.adapter.login("alice", "pw").await.unwrap();
    let submit_id = fx
        .adapter
        .submit("1001", "p-42", "A", "C++", "int main(){}")
        .await
        .expect("提交应成功");
    assert_eq!(submit_id, "123");

    let req = &server.requests()[1];
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, "/api/submit-problem-judge");
    let body: serde_json::Value = serde_json::from_str(&req.body).unwrap();
    assert_eq!(
        body["pid"], "A",
        "比赛提交的 pid 必须是展示题号（服务端按它查 contest_problem）"
    );
    assert_eq!(body["cid"], 1001);
    assert_eq!(body["code"], "int main(){}");

    server.shutdown();
}

/// 服务端 500：POST 不重试（防重复提交），错误带状态与响应体摘录。
#[tokio::test]
async fn submit_server_error_is_not_retried_and_carries_excerpt() {
    let server = LoopbackServer::start(vec![
        login_ok("jwt-1"),
        ScriptedResponse::with(500, vec![], r#"{"status":500,"msg":"duplicate"}"#),
    ])
    .await;
    let fx = fixture(server.url());
    fx.adapter.login("alice", "pw").await.unwrap();

    let err = fx
        .adapter
        .submit("1001", "p-42", "A", "C++", "code")
        .await
        .unwrap_err();
    let msg = err.user_message();
    assert!(msg.contains("500"), "错误应含状态码: {}", msg);
    assert!(msg.contains("duplicate"), "错误应含响应体摘录: {}", msg);

    let reqs = server.requests();
    assert_eq!(
        reqs.len(),
        2,
        "登录 1 次 + 提交 1 次：POST 非幂等，不得自动重试"
    );

    server.shutdown();
}
