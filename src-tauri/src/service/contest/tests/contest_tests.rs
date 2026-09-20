use super::*;

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use crate::core::entity::announcement::{Announcement, AnnouncementPage};
use crate::core::entity::contest::{Contest, ContestProblem};
use crate::core::entity::rank::{ContestRankPage, ContestRankRow, RankQuery};
use crate::core::error::AppError;
use crate::core::provider::contest::ContestProvider;
use crate::core::provider::oj_id::OjId;
use crate::core::provider::registry::ProviderSet;
use crate::core::provider::registry::ProviderRegistry;
use crate::infra::provider_registry_impl::ProviderRegistryImpl;
use crate::test_support::TempDir;

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
///
/// `mode` 可在测试中途切换（模拟「缓存命中后服务端开始 401」等时序），
/// `calls` 记录 list_contests 的调用次数，`meta_calls` / `problems_calls`
/// 分别记录 get_contest / list_contest_problems 的次数（断言缓存真的省掉请求）。
struct StubContestProvider {
    mode: RwLock<StubMode>,
    calls: AtomicUsize,
    meta_calls: AtomicUsize,
    problems_calls: AtomicUsize,
    /// `list_announcements` 返回的公告集合。
    ///
    /// 可变是为了测试「新公告检测」：同一场比赛连续拉取两次、第二次多一条公告，
    /// 服务必须只在那一次发布 `AnnouncementsPublished`。
    announcements: RwLock<Vec<Announcement>>,
}

impl StubContestProvider {
    fn new(mode: StubMode) -> Self {
        Self {
            mode: RwLock::new(mode),
            calls: AtomicUsize::new(0),
            meta_calls: AtomicUsize::new(0),
            problems_calls: AtomicUsize::new(0),
            announcements: RwLock::new(sample_announcement_page().records),
        }
    }

    fn set_mode(&self, mode: StubMode) {
        *self.mode.write().unwrap() = mode;
    }

    /// 覆盖公告集合（模拟裁判组新发布公告）。
    fn set_announcements(&self, ids: &[&str]) {
        let records = ids
            .iter()
            .map(|id| Announcement {
                id: (*id).to_string(),
                title: format!("公告 {}", id),
                content: "<p>正文</p>".to_string(),
                author: "admin".to_string(),
                created_at: 1_700_000_000,
                updated_at: 1_700_000_000,
            })
            .collect();
        *self.announcements.write().unwrap() = records;
    }

    fn current_mode(&self) -> StubMode {
        *self.mode.read().unwrap()
    }

    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    /// get_contest（比赛元信息）调用次数
    fn meta_call_count(&self) -> usize {
        self.meta_calls.load(Ordering::SeqCst)
    }

    /// list_contest_problems（题目列表）调用次数
    fn problems_call_count(&self) -> usize {
        self.problems_calls.load(Ordering::SeqCst)
    }
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
        oi_rank_score_type: None,
    }
}

fn sample_problem() -> ContestProblem {
    ContestProblem {
        id: 1,
        display_id: "A".into(),
        cid: "1011".to_string(),
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

fn sample_announcement_page() -> AnnouncementPage {
    AnnouncementPage {
        records: vec![Announcement {
            id: "9001".into(),
            title: "开赛通知".into(),
            content: "<p>比赛已开始</p>".into(),
            author: "admin".into(),
            created_at: 1_700_000_000,
            updated_at: 1_700_000_100,
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
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.current_mode() {
            StubMode::Ok => Ok(vec![sample_contest()]),
            m => Err(m.into_err("stub list_contests")),
        }
    }

    async fn get_contest(&self, _contest_id: &str) -> AppResult<Contest> {
        self.meta_calls.fetch_add(1, Ordering::SeqCst);
        match self.current_mode() {
            StubMode::Ok => Ok(sample_contest()),
            m => Err(m.into_err("stub get_contest")),
        }
    }

    async fn list_contest_problems(&self, _contest_id: &str) -> AppResult<Vec<ContestProblem>> {
        self.problems_calls.fetch_add(1, Ordering::SeqCst);
        match self.current_mode() {
            StubMode::Ok => Ok(vec![sample_problem()]),
            m => Err(m.into_err("stub list_contest_problems")),
        }
    }

    async fn get_contest_rank(
        &self,
        _contest_id: &str,
        _query: &RankQuery,
    ) -> AppResult<ContestRankPage> {
        match self.current_mode() {
            StubMode::Ok => Ok(sample_rank_page()),
            m => Err(m.into_err("stub get_contest_rank")),
        }
    }

    async fn list_announcements(
        &self,
        _contest_id: &str,
        _current_page: i64,
        _limit: i64,
    ) -> AppResult<AnnouncementPage> {
        match self.current_mode() {
            StubMode::Ok => {
                let records = self.announcements.read().unwrap().clone();
                Ok(AnnouncementPage {
                    total: records.len() as i64,
                    size: 50,
                    current: 1,
                    pages: 1,
                    records,
                })
            }
            m => Err(m.into_err("stub list_announcements")),
        }
    }
}

/// 构造基于独立临时目录的 ContestService；返回服务、Stub 句柄与目录守卫。
///
/// 第三个元素是 `TempDir` 守卫：调用点已有的 `_dir` 绑定会把它持有到用例结束，
/// 目录随之在 `Drop` 时回收（此前只在开始时清理，实测单次全量测试留下 700+ 个残留）。
fn make_service(mode: StubMode) -> (ContestService, Arc<StubContestProvider>, TempDir) {
    let dir = unique_temp_dir("contest");
    let (service, provider, _bus) = build_service_in(&dir, mode);
    (service, provider, dir)
}

/// 独立临时目录（每次调用一个，避免并行测试相互干扰）。
fn unique_temp_dir(tag: &str) -> TempDir {
    TempDir::unique(&format!("hinina-test-{tag}"))
}

/// 构造基于**指定目录**的 ContestService —— 磁盘缓存跨实例用例需共用同一目录。
///
/// 目录守卫由调用方持有：跨实例用例要在两次构造之间保住同一个目录。
fn make_service_in(dir: &TempDir, mode: StubMode) -> (ContestService, Arc<StubContestProvider>) {
    let (service, provider, _bus) = build_service_in(dir, mode);
    (service, provider)
}

/// 同 `make_service`，但把 EventBus 也交出来（断言事件发布契约）。
fn make_service_with_bus(
    mode: StubMode,
) -> (
    ContestService,
    Arc<StubContestProvider>,
    Arc<EventBus>,
    TempDir,
) {
    let dir = unique_temp_dir("contest-events");
    let (service, provider, bus) = build_service_in(&dir, mode);
    (service, provider, bus, dir)
}

fn build_service_in(
    dir: &TempDir,
    mode: StubMode,
) -> (ContestService, Arc<StubContestProvider>, Arc<EventBus>) {
    let provider = Arc::new(StubContestProvider::new(mode));
    let registry: Arc<dyn ProviderRegistry> = Arc::new(ProviderRegistryImpl::new(OjId::new("HOJ")));
    registry.register(
        OjId::new("HOJ"),
        ProviderSet {
            contest: Some(Arc::clone(&provider) as Arc<dyn ContestProvider>),
            ..Default::default()
        },
    );
    let bus = Arc::new(EventBus::new());
    let service = ContestService::new(
        registry,
        Arc::clone(&bus),
        Arc::new(Storage::new(dir.to_path_buf())),
    );
    (service, provider, bus)
}

/// 订阅 Contest 类事件并收集，供事件契约断言。
fn collect_contest_events(bus: &Arc<EventBus>) -> Arc<std::sync::Mutex<Vec<ContestEvent>>> {
    let events: Arc<std::sync::Mutex<Vec<ContestEvent>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);
    bus.subscribe(
        EventCategory::Contest,
        Arc::new(move |event: &AppEvent| {
            if let AppEvent::Contest(contest) = event {
                sink.lock().unwrap().push(contest.clone());
            }
        }),
    );
    events
}

// ── 错误变体必须穿透 Service 层 ──
//
// 变体是前端 sessionGuard 判定会话失效的唯一依据（见 core/error.rs 的 context() 文档）。
// 若 Service 用 AppError::Contest(format!(...)) 重新包装，token 过期时：
// 榜单静默 stale、提交只弹一条错误文案、选手永远不被带回登录页。
// get_rank 尤其关键 —— 它是全场最高频的认证调用（每 10s 一次）。

#[test]
fn get_rank_preserves_auth_variant() {
    let (service, _stub, _dir) = make_service(StubMode::Auth);
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
    let (service, _stub, _dir) = make_service(StubMode::Network);
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
    let (service, _stub, _dir) = make_service(StubMode::Auth);
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
    let (service, _stub, _dir) = make_service(StubMode::Auth);
    let err = block_on(service.load_contest_with_problems("1011", None))
        .expect_err("token 过期应报错");
    assert!(
        matches!(err, AppError::Auth(_)),
        "进场时 401 必须触发会话守卫，实际 {:?}",
        err
    );
}

#[test]
fn list_announcements_preserves_auth_variant() {
    let (service, _stub, _dir) = make_service(StubMode::Auth);
    let err = block_on(service.list_announcements("1011", 1, 50))
        .expect_err("token 过期应报错");
    assert!(
        matches!(err, AppError::Auth(_)),
        "公告拉取失败必须保留 Auth 变体，实际 {:?}",
        err
    );
    assert!(
        err.user_message().contains("获取比赛公告"),
        "应补上环节名: {}",
        err.user_message()
    );
}

#[test]
fn list_announcements_preserves_network_variant() {
    let (service, _stub, _dir) = make_service(StubMode::Network);
    let err = block_on(service.list_announcements("1011", 1, 50))
        .expect_err("网络异常应报错");
    assert!(
        matches!(err, AppError::Network(_)),
        "网络异常不应被改写成 Contest 变体，实际 {:?}",
        err
    );
}

// ── 正常路径 ──

#[test]
fn get_rank_returns_page_on_success() {
    let (service, _stub, _dir) = make_service(StubMode::Ok);
    let page =
        block_on(service.get_rank("1011", &RankQuery::default())).expect("成功路径不应报错");
    assert_eq!(page.records.len(), 1);
    assert_eq!(page.records[0].uid, "u1");
}

#[test]
fn load_contest_with_problems_returns_bundle_and_selects() {
    let (service, _stub, _dir) = make_service(StubMode::Ok);
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

#[test]
fn list_announcements_returns_page_on_success() {
    let (service, _stub, _dir) = make_service(StubMode::Ok);
    let page = block_on(service.list_announcements("1011", 1, 50)).expect("成功路径不应报错");
    assert_eq!(page.records.len(), 1);
    assert_eq!(page.records[0].id, "9001");
    assert_eq!(page.records[0].author, "admin");
}

// ── 新公告检测（红点提醒的事件源）──
//
// 公告红点必须由事件驱动：前端仍按 60s 节拍拉取，但「有新公告」这一状态变更
// 走 EventBus，由 main.rs 的事件桥转发到 webview。以下锁定三个不变量。

/// 从收集到的事件里筛出 AnnouncementsPublished
fn published_ids(events: &Arc<std::sync::Mutex<Vec<ContestEvent>>>) -> Vec<Vec<String>> {
    events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|e| match e {
            ContestEvent::AnnouncementsPublished { new_ids, .. } => Some(new_ids.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn first_announcement_fetch_establishes_baseline_without_event() {
    // 首次拉取没有基线可比：发事件等于一开机就给每位选手亮红点
    let (service, _stub, bus, _dir) = make_service_with_bus(StubMode::Ok);
    let events = collect_contest_events(&bus);

    block_on(service.list_announcements("1011", 1, 50)).expect("首次拉取应成功");

    assert!(
        published_ids(&events).is_empty(),
        "首次拉取不应发布新公告事件，实际 {:?}",
        published_ids(&events)
    );
}

#[test]
fn new_announcement_publishes_event_once() {
    let (service, stub, bus, _dir) = make_service_with_bus(StubMode::Ok);
    let events = collect_contest_events(&bus);

    // 建基线
    block_on(service.list_announcements("1011", 1, 50)).expect("首次拉取应成功");

    // 裁判组发布 9002
    stub.set_announcements(&["9001", "9002"]);
    block_on(service.list_announcements("1011", 1, 50)).expect("二次拉取应成功");

    assert_eq!(
        published_ids(&events),
        vec![vec!["9002".to_string()]],
        "应且仅应发布一次新公告事件，且只带新增 ID"
    );

    // 再拉一次（列表不变）：不得重复发事件，否则红点会被反复点亮
    block_on(service.list_announcements("1011", 1, 50)).expect("三次拉取应成功");
    assert_eq!(published_ids(&events).len(), 1, "同一批公告不得重复发事件");
}

#[test]
fn announcement_baseline_is_isolated_per_contest() {
    // 基线按比赛隔离：只看过 1011 的情况下首次看 1012 不应把 1012 的既有公告
    // 当成「新公告」（同一进程内切换比赛是常态）
    let (service, stub, bus, _dir) = make_service_with_bus(StubMode::Ok);
    let events = collect_contest_events(&bus);

    block_on(service.list_announcements("1011", 1, 50)).expect("1011 首次拉取应成功");
    block_on(service.list_announcements("1012", 1, 50)).expect("1012 首次拉取应成功");
    assert!(published_ids(&events).is_empty(), "各自首次拉取都不应发事件");

    // 1012 新增一条：只影响 1012
    stub.set_announcements(&["9001", "9002"]);
    block_on(service.list_announcements("1012", 1, 50)).expect("1012 二次拉取应成功");

    let published = published_ids(&events);
    assert_eq!(published.len(), 1, "只有 1012 应发事件");
    assert_eq!(published[0], vec!["9002".to_string()]);
}

#[test]
fn failed_fetch_keeps_baseline_so_next_success_still_reports() {
    // 拉取失败时不得推进基线：否则失败期间发布的公告会被永久漏报
    let (service, stub, bus, _dir) = make_service_with_bus(StubMode::Ok);
    let events = collect_contest_events(&bus);

    block_on(service.list_announcements("1011", 1, 50)).expect("首次拉取应成功");

    stub.set_mode(StubMode::Network);
    let _ = block_on(service.list_announcements("1011", 1, 50));
    stub.set_mode(StubMode::Ok);

    stub.set_announcements(&["9001", "9002"]);
    block_on(service.list_announcements("1011", 1, 50)).expect("恢复后拉取应成功");

    assert_eq!(
        published_ids(&events),
        vec![vec!["9002".to_string()]],
        "失败期间的基线必须保留，恢复后仍应报出新增公告"
    );
}

// ── 比赛列表 TTL 缓存语义 ──

#[test]
fn list_contests_cache_hit_skips_second_provider_call() {
    let (service, stub, _dir) = make_service(StubMode::Ok);

    let first = block_on(service.list_contests(60)).expect("首次拉取应成功");
    let second = block_on(service.list_contests(60)).expect("缓存命中应成功");

    assert_eq!(stub.call_count(), 1, "TTL 内第二次调用不应再打 Provider");
    assert_eq!(first.len(), second.len());
}

#[test]
fn list_contests_expired_cache_refetches() {
    let (service, stub, _dir) = make_service(StubMode::Ok);

    block_on(service.list_contests(60)).expect("首次拉取应成功");
    // TTL=0：缓存立即视为过期，必须重新请求
    block_on(service.list_contests(0)).expect("过期后重新拉取应成功");

    assert_eq!(stub.call_count(), 2, "缓存过期后应重新请求 Provider");
}

#[test]
fn refresh_forces_provider_call_even_within_ttl() {
    let (service, stub, _dir) = make_service(StubMode::Ok);

    block_on(service.list_contests(60)).expect("首次拉取应成功");
    block_on(service.refresh()).expect("refresh 应成功");

    assert_eq!(stub.call_count(), 2, "refresh 必须跳过缓存强制拉取");
}

#[test]
fn failed_refresh_leaves_no_stale_cache() {
    // refresh 先清缓存再拉取：拉取失败后不得残留旧数据，
    // 下一次调用必须重新打到 Provider（而不是静默返回过期列表）
    let (service, stub, _dir) = make_service(StubMode::Ok);
    block_on(service.list_contests(60)).expect("首次拉取应成功");
    assert_eq!(stub.call_count(), 1);

    stub.set_mode(StubMode::Auth);
    let err = block_on(service.refresh()).expect_err("刷新失败应报错");
    assert!(matches!(err, AppError::Auth(_)), "实际 {:?}", err);

    stub.set_mode(StubMode::Ok);
    block_on(service.list_contests(60)).expect("恢复后应重新拉取成功");
    assert_eq!(
        stub.call_count(),
        3,
        "失败的 refresh 已清空缓存，恢复后必须重新请求而不是吃旧缓存"
    );
}

// ── 比赛元信息缓存（内存 + 磁盘，TTL 120s）──

#[test]
fn meta_cache_hit_skips_second_get_contest() {
    let (service, stub, _dir) = make_service(StubMode::Ok);

    block_on(service.load_contest_with_problems("1011", None)).expect("首次加载应成功");
    block_on(service.load_contest_with_problems("1011", None)).expect("二次加载应成功");

    assert_eq!(stub.meta_call_count(), 1, "元信息应命中缓存，不再请求 Provider");
    assert_eq!(
        stub.problems_call_count(),
        2,
        "题目列表必须每次实时拉取（ac/total 是轮询存在的理由）"
    );
}

#[test]
fn meta_cache_survives_new_service_instance() {
    // 磁盘缓存：新实例（模拟重启）仍能命中，且 TTL 依据落盘的 fetchedAt 继续计时
    let dir = TempDir::named(&format!(
        "hinina-test-contest-meta-persist-{}",
        std::process::id()
    ));

    let (service, stub) = make_service_in(&dir, StubMode::Ok);
    block_on(service.load_contest_with_problems("1011", None)).expect("首次加载应成功");
    assert_eq!(stub.meta_call_count(), 1);
    drop(service);

    let (reopened, stub2) = make_service_in(&dir, StubMode::Ok);
    block_on(reopened.load_contest_with_problems("1011", None)).expect("重启后加载应成功");

    assert_eq!(stub2.meta_call_count(), 0, "元信息应命中磁盘缓存");
    assert_eq!(stub2.problems_call_count(), 1, "题目列表仍实时拉取");
}

#[test]
fn meta_cache_isolates_contests() {
    let (service, stub, _dir) = make_service(StubMode::Ok);

    block_on(service.load_contest_with_problems("1011", None)).expect("加载 1011 应成功");
    block_on(service.load_contest_with_problems("1012", None)).expect("加载 1012 应成功");

    assert_eq!(stub.meta_call_count(), 2, "不同比赛的元信息不得互相命中");
}

#[test]
fn refresh_clears_meta_cache() {
    let (service, stub, _dir) = make_service(StubMode::Ok);

    block_on(service.load_contest_with_problems("1011", None)).expect("首次加载应成功");
    block_on(service.refresh()).expect("refresh 应成功");
    block_on(service.load_contest_with_problems("1011", None)).expect("刷新后加载应成功");

    assert_eq!(
        stub.meta_call_count(),
        2,
        "refresh 必须同时清掉元信息缓存（内存 + 磁盘）"
    );
}

#[test]
fn meta_cache_never_stores_errors() {
    // 只缓存成功结果：首次 401 不得入缓存，否则会话恢复后仍返回旧错误
    let (service, stub, _dir) = make_service(StubMode::Auth);

    let err = block_on(service.load_contest_with_problems("1011", None))
        .expect_err("token 过期应报错");
    assert!(matches!(err, AppError::Auth(_)), "变体必须保留，实际 {:?}", err);

    stub.set_mode(StubMode::Ok);
    block_on(service.load_contest_with_problems("1011", None)).expect("恢复后应成功");
    assert_eq!(stub.meta_call_count(), 2, "错误不得入缓存，恢复后必须重新请求");
}

// ── 公告已读状态（客户端本地特性）──

#[test]
fn read_state_roundtrip_merges_and_dedupes() {
    let (service, _stub, dir) = make_service(StubMode::Ok);

    // 初始无文件：空列表而不是报错
    assert_eq!(
        service.get_read_announcement_ids("1011", "uid-1").unwrap(),
        Vec::<String>::new()
    );

    service
        .mark_announcements_read("1011", "uid-1", &["a".into(), "b".into()])
        .expect("标记已读应成功");
    // 重复 + 新增：合并去重，保留首次出现顺序
    service
        .mark_announcements_read("1011", "uid-1", &["b".into(), "c".into()])
        .expect("标记已读应成功");

    let ids = service
        .get_read_announcement_ids("1011", "uid-1")
        .expect("读取应成功");
    assert_eq!(ids, vec!["a".to_string(), "b".to_string(), "c".to_string()]);

    // 落盘位置与格式锁定：announcements_read/{cid}_{uid}.json + {"readIds":[...]}
    let file = dir.join("announcements_read").join("1011_uid-1.json");
    assert!(file.exists(), "已读状态应持久化到 {:?}", file);
    let raw = std::fs::read_to_string(&file).unwrap();
    let value: serde_json::Value = serde_json::from_str(&raw).expect("应为合法 JSON");
    assert_eq!(value["readIds"].as_array().map(|a| a.len()), Some(3));

    // 不同用户互相隔离
    assert_eq!(
        service.get_read_announcement_ids("1011", "uid-2").unwrap(),
        Vec::<String>::new()
    );
}

#[test]
fn read_state_corrupt_file_degrades_to_empty() {
    let (service, _stub, dir) = make_service(StubMode::Ok);

    let read_dir = dir.join("announcements_read");
    std::fs::create_dir_all(&read_dir).unwrap();
    std::fs::write(read_dir.join("1011_uid-1.json"), "not-json{{{").unwrap();

    // 损坏文件只告警降级，绝不阻断公告展示
    let ids = service
        .get_read_announcement_ids("1011", "uid-1")
        .expect("损坏文件应降级为空列表而不是报错");
    assert!(ids.is_empty());

    // 损坏后仍可正常标记（重建文件）
    service
        .mark_announcements_read("1011", "uid-1", &["x".into()])
        .expect("标记应成功");
    assert_eq!(
        service.get_read_announcement_ids("1011", "uid-1").unwrap(),
        vec!["x".to_string()]
    );
}

#[test]
fn read_state_rejects_path_separators() {
    let (service, _stub, _dir) = make_service(StubMode::Ok);

    // uid / cid 来自会话与前端入参，含路径分隔符或 .. 时必须拒绝，防止写出存储根目录之外
    for bad in ["../evil", "a/b", "a\\b", ""] {
        let err = service
            .get_read_announcement_ids("1011", bad)
            .expect_err(&format!("非法 uid {:?} 应被拒绝", bad));
        assert!(matches!(err, AppError::Io(_)), "实际 {:?}", err);

        let err = service
            .mark_announcements_read(bad, "uid-1", &["a".into()])
            .expect_err(&format!("非法 cid {:?} 应被拒绝", bad));
        assert!(matches!(err, AppError::Io(_)), "实际 {:?}", err);
    }
}


// ── OJSwitched：OJ 域缓存失效（键控不含 OJ 维度，切 OJ 防跨 OJ 撞号）──

#[test]
fn contest_meta_cache_key_carries_oj_scope_so_cross_oj_never_hits() {
    // 「延迟清理磁盘缓存」安全的前提：键带 OJ 维度 → 跨 OJ 结构上不可能命中
    use crate::core::event::event_bus::EventBus;

    let dir = TempDir::named("hinina-test-contest-oj-scope");
    let provider = Arc::new(StubContestProvider::new(StubMode::Ok));
    let registry: Arc<dyn ProviderRegistry> = Arc::new(ProviderRegistryImpl::new(OjId::new("HOJ")));
    for id in ["HOJ", "QDUOJ"] {
        registry.register(
            OjId::new(id),
            ProviderSet {
                contest: Some(Arc::clone(&provider) as Arc<dyn ContestProvider>),
                ..Default::default()
            },
        );
    }
    let service = ContestService::new(
        Arc::clone(&registry),
        Arc::new(EventBus::new()),
        Arc::new(Storage::new(dir.to_path_buf())),
    );

    // HOJ：首拉 + 二次命中
    block_on(service.load_contest_meta("7")).expect("HOJ 元信息失败");
    assert_eq!(provider.meta_call_count(), 1);
    block_on(service.load_contest_meta("7")).expect("HOJ 元信息二次失败");
    assert_eq!(provider.meta_call_count(), 1, "同一 OJ 应命中缓存");

    // 切 OJ：同一 cid 必须重新请求
    registry.set_current(OjId::new("QDUOJ"));
    block_on(service.load_contest_meta("7")).expect("换 OJ 后元信息失败");
    assert_eq!(
        provider.meta_call_count(),
        2,
        "跨 OJ 不得命中同一键（键缺 OJ 维度会让旧 OJ 的比赛元信息驱动新 OJ 查询）"
    );
}

#[test]
fn oj_switched_clears_contest_scoped_caches() {
    use crate::core::event::app_event::{AppEvent, SystemEvent};

    let dir = TempDir::named("hinina-test-contest-oj-switch");
    let bus = Arc::new(EventBus::new());
    let provider = Arc::new(StubContestProvider::new(StubMode::Ok));
    let registry: Arc<dyn ProviderRegistry> = Arc::new(ProviderRegistryImpl::new(OjId::new("HOJ")));
    registry.register(
        OjId::new("HOJ"),
        ProviderSet {
            contest: Some(Arc::clone(&provider) as Arc<dyn ContestProvider>),
            ..Default::default()
        },
    );
    let service = ContestService::new(
        registry,
        Arc::clone(&bus),
        Arc::new(Storage::new(dir.to_path_buf())),
    );

    // 预置三层缓存：列表（内存）+ 元信息（内存 + **磁盘**）。
    // 磁盘条目必须真实落盘 —— 否则「目录不存在」的断言恒真，等于零覆盖
    *service.cache.write().unwrap() = Some(ContestCache {
        contests: vec![],
        fetched_at: Instant::now(),
    });
    block_on(service.load_contest_meta("7")).expect("预置元信息失败");
    let disk_entry = dir.join("cache").join("contest_meta").join("HOJ").join("7.json");
    assert!(disk_entry.exists(), "预置失败：元信息磁盘缓存未落盘");
    assert!(!service.meta_cache.is_empty(), "预置失败：元信息内存缓存为空");

    bus.publish(&AppEvent::System(SystemEvent::OJSwitched { oj_id: "QDUOJ".into() }));

    assert!(service.cache.read().unwrap().is_none(), "列表缓存应被清空");
    assert!(service.meta_cache.is_empty(), "元信息内存缓存应被清空");
    // 磁盘段是延迟投递（I/O 不阻塞发布方）：等队列排空后再断言。
    // 删掉 subscribe_deferred 注册后本断言必须失败 —— 这是延迟清理的有效回归覆盖
    bus.flush_deferred();
    assert!(
        !dir.join("cache/contest_meta").exists(),
        "磁盘元信息缓存应被延迟清理"
    );
}

// ── 设置页「清空缓存」（同步清理，不重拉） ──

#[test]
fn clear_caches_empties_list_memory_and_disk_caches() {
    let (service, provider, dir) = make_service(StubMode::Ok);

    // 预置三层：列表（内存）+ 元信息（内存 + **磁盘**）。磁盘条目必须真实落盘
    // —— 否则「目录不存在」的断言恒真，等于零覆盖
    *service.cache.write().unwrap() = Some(ContestCache {
        contests: vec![],
        fetched_at: Instant::now(),
    });
    block_on(service.load_contest_meta("7")).expect("预置元信息失败");
    assert!(
        dir.join("cache/contest_meta/HOJ/7.json").exists(),
        "预置失败：元信息磁盘缓存未落盘"
    );
    assert!(!service.meta_cache.is_empty(), "预置失败：元信息内存缓存为空");
    let calls_before = provider.meta_call_count();

    service.clear_caches();

    assert!(service.cache.read().unwrap().is_none(), "列表缓存应被清空");
    assert!(service.meta_cache.is_empty(), "元信息内存缓存应被清空");
    assert!(
        !dir.join("cache/contest_meta").exists(),
        "元信息磁盘缓存应被同步清空（用户点了按钮就该等到真清完）"
    );
    assert_eq!(
        provider.meta_call_count(),
        calls_before,
        "清空缓存不得顺带发请求（补拉时机由调用方决定）"
    );

    // 清完再取必须真的回源 —— 否则「清了但没生效」
    block_on(service.load_contest_meta("7")).expect("清空后重新获取失败");
    assert_eq!(
        provider.meta_call_count(),
        calls_before + 1,
        "清空后下一次查询必须回源"
    );
}

#[test]
fn clear_caches_keeps_announcement_baseline() {
    // 公告基线**不是缓存**，而是「已经告诉过用户哪些公告」的记忆：清掉它会让
    // 清空之后新发布的公告在下一次拉取时被当成「首次拉取」而**漏报**（红点不亮）。
    let (service, provider, bus, _dir) = make_service_with_bus(StubMode::Ok);
    let events = collect_contest_events(&bus);

    block_on(service.list_announcements("1012", 1, 20)).expect("首次拉取公告失败");
    assert!(
        events.lock().unwrap().is_empty(),
        "首次拉取只建基线，不该发事件"
    );

    service.clear_caches();

    // 清空之后裁判组补发了一条公告
    provider.set_announcements(&["9001", "9002"]);
    block_on(service.list_announcements("1012", 1, 20)).expect("二次拉取公告失败");

    assert_eq!(
        published_ids(&events),
        vec![vec!["9002".to_string()]],
        "清空缓存不得丢掉公告基线（丢掉 = 清空后新增的公告漏报，红点不亮）"
    );
}

// ── 重置客户端（清缓存 + 忘掉公告基线 + 清已读状态） ──

#[test]
fn clear_announcement_baseline_forgets_baseline() {
    // 与上一个用例相反的一侧：**重置**语义下必须忘掉基线 —— 忘掉后下一次拉取
    // 等价于「首次拉取」（静默重建基线，不报新公告）。两条用例成对存在，
    // 才能锁住「clear_caches 保留 / clear_announcement_baseline 清空」这个区别。
    let (service, provider, bus, _dir) = make_service_with_bus(StubMode::Ok);
    let events = collect_contest_events(&bus);

    block_on(service.list_announcements("1012", 1, 20)).expect("首次拉取公告失败");
    assert!(events.lock().unwrap().is_empty());

    service.clear_announcement_baseline();

    provider.set_announcements(&["9001", "9002"]);
    block_on(service.list_announcements("1012", 1, 20)).expect("二次拉取公告失败");

    assert!(
        events.lock().unwrap().is_empty(),
        "基线已忘：下次拉取应重新建基线（等价首次），而不是报新公告"
    );
}

#[test]
fn clear_announcement_read_state_removes_files_and_is_idempotent() {
    let (service, _provider, dir) = make_service(StubMode::Ok);

    service
        .mark_announcements_read("1012", "uid-1", &["9001".to_string()])
        .expect("标记已读失败");
    assert_eq!(
        service
            .get_read_announcement_ids("1012", "uid-1")
            .expect("读取已读状态失败"),
        vec!["9001".to_string()],
        "前置条件：已读状态应已落盘"
    );
    assert!(dir.join("announcements_read").exists());

    assert!(
        service.clear_announcement_read_state(),
        "目录存在时应报告确实删除了"
    );

    assert!(
        service
            .get_read_announcement_ids("1012", "uid-1")
            .expect("清空后读取失败")
            .is_empty(),
        "已读状态应被清空（表现 = 红点全部复亮）"
    );
    assert!(!dir.join("announcements_read").exists());

    // 幂等：目录已不在时返回 false（「本就没有」不是失败），不报错
    assert!(!service.clear_announcement_read_state());
}
