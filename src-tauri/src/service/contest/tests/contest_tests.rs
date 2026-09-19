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
}

impl StubContestProvider {
    fn new(mode: StubMode) -> Self {
        Self {
            mode: RwLock::new(mode),
            calls: AtomicUsize::new(0),
            meta_calls: AtomicUsize::new(0),
            problems_calls: AtomicUsize::new(0),
        }
    }

    fn set_mode(&self, mode: StubMode) {
        *self.mode.write().unwrap() = mode;
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
            StubMode::Ok => Ok(sample_announcement_page()),
            m => Err(m.into_err("stub list_announcements")),
        }
    }
}

/// 构造基于独立临时目录的 ContestService；返回服务、Stub 句柄与目录。
fn make_service(mode: StubMode) -> (ContestService, Arc<StubContestProvider>, std::path::PathBuf) {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "hinina-test-contest-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    make_service_in(dir, mode)
}

/// 构造基于**指定目录**的 ContestService —— 磁盘缓存跨实例用例需共用同一目录。
fn make_service_in(
    dir: std::path::PathBuf,
    mode: StubMode,
) -> (ContestService, Arc<StubContestProvider>, std::path::PathBuf) {
    let provider = Arc::new(StubContestProvider::new(mode));
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
        Arc::new(EventBus::new()),
        Arc::new(Storage::new(dir.clone())),
    );
    (service, provider, dir)
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
    let dir = std::env::temp_dir().join(format!(
        "hinina-test-contest-meta-persist-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);

    let (service, stub, dir) = make_service_in(dir, StubMode::Ok);
    block_on(service.load_contest_with_problems("1011", None)).expect("首次加载应成功");
    assert_eq!(stub.meta_call_count(), 1);
    drop(service);

    let (reopened, stub2, _dir) = make_service_in(dir, StubMode::Ok);
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

    let dir = std::env::temp_dir().join("hinina-test-contest-oj-scope");
    let _ = std::fs::remove_dir_all(&dir);
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
        Arc::new(Storage::new(dir.clone())),
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

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn oj_switched_clears_contest_scoped_caches() {
    use crate::core::event::app_event::{AppEvent, SystemEvent};

    let dir = std::env::temp_dir().join("hinina-test-contest-oj-switch");
    let _ = std::fs::remove_dir_all(&dir);
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
    let service = ContestService::new(registry, Arc::clone(&bus), Arc::new(Storage::new(dir.clone())));

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

    let _ = std::fs::remove_dir_all(&dir);
}
