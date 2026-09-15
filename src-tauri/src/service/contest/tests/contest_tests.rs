use super::*;

use std::collections::HashMap;
use std::sync::Arc;

use crate::core::entity::contest::{Contest, ContestProblem};
use crate::core::entity::rank::{ContestRankPage, ContestRankRow, RankQuery};
use crate::core::error::AppError;
use crate::core::provider::contest::ContestProvider;
use crate::core::provider::oj_type::OJType;
use crate::core::provider::registry::ProviderRegistry;
use crate::infra::provider_registry_impl::ProviderRegistryImpl;

/// 在当前线程创建独立 tokio runtime，避免嵌套 runtime panic。
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("创建测试 runtime 失败");
    rt.block_on(fut)
}

/// Stub 行为：成功，或以指定变体失败。
#[derive(Clone, Copy)]
enum StubMode {
    Ok,
    /// 模拟 token 过期（HTTP 401，或 HOJ 体内 403「请您先登录」）
    Auth,
    /// 模拟网络中断
    Network,
}

impl StubMode {
    fn into_err(self, ctx: &str) -> AppError {
        match self {
            StubMode::Auth => AppError::Auth(format!("{}: HTTP 401 Unauthorized", ctx)),
            StubMode::Network => AppError::Network(format!("{}: connection reset", ctx)),
            StubMode::Ok => unreachable!("Ok 模式不应构造错误"),
        }
    }
}

/// Stub ContestProvider：所有方法按同一模式响应，便于逐方法断言变体是否被保留。
struct StubContestProvider {
    mode: StubMode,
}

fn sample_contest() -> Contest {
    Contest {
        id: "1011".into(),
        title: "测试赛".into(),
        start_time: 1_700_000_000,
        end_time: 1_700_010_000,
        description: String::new(),
        contest_type: 0,
        status: 0,
        auth: 0,
        rank_show_name: "username".into(),
        seal_rank: false,
        seal_rank_time: None,
        allow_end_submit: false,
    }
}

fn sample_problem() -> ContestProblem {
    ContestProblem {
        id: 1,
        display_id: "A".into(),
        cid: 1011,
        problem_id: "1061".into(),
        display_title: "A + B".into(),
        ac: 3,
        total: 10,
        color: String::new(),
    }
}

fn sample_rank_page() -> ContestRankPage {
    ContestRankPage {
        records: vec![ContestRankRow {
            rank: 1,
            uid: "u1".into(),
            username: "alice".into(),
            realname: String::new(),
            nickname: String::new(),
            school: String::new(),
            gender: String::new(),
            avatar: String::new(),
            ac: 2,
            total: 3,
            total_time: 120,
            total_score: None,
            submission_info: HashMap::new(),
            time_info: HashMap::new(),
        }],
        total: 1,
        size: 50,
        current: 1,
        pages: 1,
    }
}

#[async_trait::async_trait]
impl ContestProvider for StubContestProvider {
    async fn list_contests(&self) -> AppResult<Vec<Contest>> {
        match self.mode {
            StubMode::Ok => Ok(vec![sample_contest()]),
            m => Err(m.into_err("stub list_contests")),
        }
    }

    async fn get_contest(&self, _contest_id: &str) -> AppResult<Contest> {
        match self.mode {
            StubMode::Ok => Ok(sample_contest()),
            m => Err(m.into_err("stub get_contest")),
        }
    }

    async fn list_contest_problems(&self, _contest_id: &str) -> AppResult<Vec<ContestProblem>> {
        match self.mode {
            StubMode::Ok => Ok(vec![sample_problem()]),
            m => Err(m.into_err("stub list_contest_problems")),
        }
    }

    async fn get_contest_rank(
        &self,
        _contest_id: &str,
        _query: &RankQuery,
    ) -> AppResult<ContestRankPage> {
        match self.mode {
            StubMode::Ok => Ok(sample_rank_page()),
            m => Err(m.into_err("stub get_contest_rank")),
        }
    }
}

fn make_service(mode: StubMode) -> ContestService {
    let provider = Arc::new(StubContestProvider { mode });
    let registry: Arc<dyn ProviderRegistry> = Arc::new(ProviderRegistryImpl::new(OJType::HOJ));
    registry.register_contest(OJType::HOJ, provider);
    ContestService::new(registry, Arc::new(EventBus::new()))
}

// ── 错误变体必须穿透 Service 层 ──
//
// 变体是前端 sessionGuard 判定会话失效的唯一依据（见 core/error.rs 的 context() 文档）。
// 若 Service 用 AppError::Contest(format!(...)) 重新包装，token 过期时：
// 榜单静默 stale、提交只弹一条错误文案、选手永远不被带回登录页。
// get_rank 尤其关键 —— 它是全场最高频的认证调用（每 10s 一次）。

#[test]
fn get_rank_preserves_auth_variant() {
    let service = make_service(StubMode::Auth);
    let err = block_on(service.get_rank("1011", &RankQuery::default()))
        .expect_err("token 过期应报错");
    assert!(
        matches!(err, AppError::Auth(_)),
        "get_rank 必须保留 Auth 变体，否则 401 永远不会触发会话守卫，实际 {:?}",
        err
    );
    assert!(
        err.user_message().contains("获取比赛榜单失败"),
        "应补上环节名便于排障: {}",
        err.user_message()
    );
}

#[test]
fn get_rank_preserves_network_variant() {
    let service = make_service(StubMode::Network);
    let err = block_on(service.get_rank("1011", &RankQuery::default()))
        .expect_err("网络异常应报错");
    assert!(
        matches!(err, AppError::Network(_)),
        "网络异常不应被改写成 Contest 变体，实际 {:?}",
        err
    );
}

#[test]
fn list_contests_preserves_auth_variant() {
    // 登录页匿名简报链路：失败时前端要能区分「连不上」与「凭证无效」
    let service = make_service(StubMode::Auth);
    let err = block_on(service.list_contests(0)).expect_err("token 过期应报错");
    assert!(
        matches!(err, AppError::Auth(_)),
        "实际 {:?}",
        err
    );
}

#[test]
fn load_contest_with_problems_preserves_auth_variant() {
    // 进场链路：外壳 loadContest 走的就是这个方法
    let service = make_service(StubMode::Auth);
    let err = block_on(service.load_contest_with_problems("1011", None))
        .expect_err("token 过期应报错");
    assert!(
        matches!(err, AppError::Auth(_)),
        "进场时 401 必须触发会话守卫，实际 {:?}",
        err
    );
}

// ── 正常路径 ──

#[test]
fn get_rank_returns_page_on_success() {
    let service = make_service(StubMode::Ok);
    let page =
        block_on(service.get_rank("1011", &RankQuery::default())).expect("成功路径不应报错");
    assert_eq!(page.records.len(), 1);
    assert_eq!(page.records[0].uid, "u1");
}

#[test]
fn load_contest_with_problems_returns_bundle_and_selects() {
    let service = make_service(StubMode::Ok);
    let bundle = block_on(service.load_contest_with_problems("1011", None))
        .expect("成功路径不应报错");
    assert_eq!(bundle.contest.id, "1011");
    assert_eq!(bundle.problems.len(), 1);
    assert_eq!(
        service.current_contest_id(),
        Some("1011".to_string()),
        "加载后应自动选中该比赛"
    );
}
