// `list_contest_submissions` 提取实现的契约测试（M2 工作包 A）。
//
// 核心锁定点：**`only_mine=true` 由后端强制**（产品决策「提交记录只显示本人」，
// 前端不可绕过）与分页默认值/钳制。此前这两条只存在于测试文件内复制的
// 「等价函数」里——现在直接断言脚本 server **实际收到**的查询串。

use super::*;
use crate::test_support::app_ctx::context_with_hoj;
use crate::test_support::loopback::{LoopbackServer, ScriptedResponse};

/// 测试用参数构造（结构体字段收敛点，评审 #9 配套）。
#[allow(clippy::too_many_arguments)]
fn params(
    contest_id: &str,
    current_page: Option<i64>,
    limit: Option<i64>,
    problem_display_id: Option<String>,
    status: Option<i32>,
) -> super::ListSubmissionsParams {
    super::ListSubmissionsParams {
        contest_id: contest_id.into(),
        current_page,
        limit,
        problem_display_id,
        status,
    }
}

/// HOJ 提交列表的成功响应（信封 {status,msg,data}；JudgeVO 全字段 serde(default)，
/// 空对象即可映射）。与真实协议的差异由适配器契约测试（hoj/tests/contract_tests.rs）覆盖。
fn submissions_page() -> ScriptedResponse {
    ScriptedResponse::json(
        r#"{"status":200,"msg":"success","data":{"records":[{}],"total":1,"size":20,"current":1,"pages":1}}"#,
    )
}

#[tokio::test]
async fn only_mine_is_forced_and_defaults_applied() {
    let server = LoopbackServer::start(vec![submissions_page()]).await;
    let (ctx, _dir) = context_with_hoj(server.url()).await;

    let page = list_contest_submissions_impl(&ctx, params("1001", None, None, None, None))
        .await
        .expect("默认参数应成功");

    assert_eq!(page.records.len(), 1, "响应应原样映射为分页");

    let reqs = server.requests();
    assert_eq!(reqs.len(), 1, "恰好一次列表请求");
    let path = &reqs[0].path;
    assert!(
        path.starts_with("/api/contest-submissions?"),
        "路径: {}",
        path
    );
    assert!(
        path.contains("onlyMine=true"),
        "only_mine 必须强制为 true: {}",
        path
    );
    assert!(path.contains("currentPage=1"), "默认第 1 页: {}", path);
    assert!(path.contains("limit=20"), "默认每页 20: {}", path);
    assert!(path.contains("contestID=1001"), "比赛 ID 透传: {}", path);

    server.shutdown();
}

/// 非法分页（0/负）与空白筛选：钳制到 1 / 视为不筛选，不拼进查询串。
#[tokio::test]
async fn paging_clamps_and_blank_filter_is_dropped() {
    let server = LoopbackServer::start(vec![submissions_page()]).await;
    let (ctx, _dir) = context_with_hoj(server.url()).await;

    list_contest_submissions_impl(
        &ctx,
        params("1001", Some(-5), Some(0), Some("   ".into()), None),
    )
    .await
    .expect("非法入参应被钳制而不是报错");

    let path = &server.requests()[0].path;
    assert!(path.contains("currentPage=1"), "负页码钳制到 1: {}", path);
    assert!(path.contains("limit=1"), "0 limit 钳制到 1: {}", path);
    assert!(
        !path.contains("problemID"),
        "纯空白筛选不得拼进查询串: {}",
        path
    );

    server.shutdown();
}

/// 有效筛选透传：display_id 与 status 原样进入查询串。
#[tokio::test]
async fn optional_filters_pass_through() {
    let server = LoopbackServer::start(vec![submissions_page()]).await;
    let (ctx, _dir) = context_with_hoj(server.url()).await;

    list_contest_submissions_impl(
        &ctx,
        params("1001", Some(3), Some(100), Some("A".into()), Some(-1)),
    )
    .await
    .unwrap();

    let path = &server.requests()[0].path;
    assert!(path.contains("currentPage=3"), "{}", path);
    assert!(path.contains("limit=100"), "{}", path);
    assert!(path.contains("problemID=A"), "{}", path);
    assert!(path.contains("status=-1"), "{}", path);

    server.shutdown();
}
