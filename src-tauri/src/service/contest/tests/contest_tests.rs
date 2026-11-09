use super::*;

#[test]
fn reselect_same_contest_keeps_scope_but_aba_and_oj_switch_revoke_it() {
    let (service, _provider, _dir) = make_service(StubMode::Ok);
    let access = service.access.clone();
    service.select_contest("a").unwrap();
    let snapshot = access.snapshot().unwrap();
    service.select_contest("a").unwrap();
    assert!(!snapshot.cancellation.is_cancelled());
    service.select_contest("b").unwrap();
    service.select_contest("a").unwrap();
    assert!(snapshot.cancellation.is_cancelled());
    service.on_oj_switched();
    assert!(service.current_contest_id().is_none());
}

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use crate::core::entity::announcement::{Announcement, AnnouncementPage};
use crate::core::entity::contest::{Contest, ContestProblem};
use crate::core::entity::rank::{ContestRankPage, ContestRankRow, RankQuery};
use crate::core::error::AppError;
use crate::core::provider::contest::ContestProvider;
use crate::core::provider::oj_id::OjId;
use crate::core::provider::registry::ProviderRegistry;
use crate::core::provider::registry::ProviderSet;
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
    /// 模拟「需凭证进入」类业务拒绝（is_credential_rejection 判定为 true 的形态）
    Contest,
    /// 无视 `registered` 的业务拒绝：报名后题目仍失败（模拟服务端状态未解锁），
    /// 用于走通服务级**成功记忆**路径
    ContestAlways,
}

impl StubMode {
    fn into_err(self, ctx: &str) -> AppError {
        match self {
            StubMode::Auth => AppError::Auth(format!("{}: HTTP 401 Unauthorized", ctx)),
            StubMode::Network => AppError::Network(format!("{}: connection reset", ctx)),
            StubMode::Contest | StubMode::ContestAlways => {
                AppError::Contest(format!("{}: 私有赛未报名，需要比赛密码", ctx))
            }
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
    /// 服务必须只在那一次发布 `CoreEvent::AnnouncementChanged`。
    announcements: RwLock<Vec<Announcement>>,
    /// `get_contest` 返回的比赛 auth（0=公开，1=私有），测私有赛进入分支用。
    contest_auth: RwLock<i32>,
    /// `list_contest_problems` 的独立模式（`None` 时跟随全局 `mode`）。
    ///
    /// 错误驱动的私有赛进入用例需要「元信息成功 + 题目列表业务失败」的组合，
    /// 全局 `mode` 做不到（会连 `get_contest` 一起失败）。
    problems_mode: RwLock<Option<StubMode>>,
    /// 是否已报名（`ensure_contest_access` 返回 Ok 时置位）。
    ///
    /// 模拟服务端事实：报名解锁私有赛题目列表 —— `problems_mode` 为 Contest 且
    /// 未报名时题目列表失败，报名后（或 `problems_mode` 放开）成功。
    registered: AtomicBool,
    /// `ensure_contest_access` 的行为与调用记录。
    access_mode: RwLock<AccessMode>,
    access_calls: AtomicUsize,
    /// 最近一次进入调用收到的凭证（断言配置密码确实透传到 provider）。
    access_credential: RwLock<Option<String>>,
    /// `ensure_contest_access` 的延迟（毫秒）：复现「清理发生在途」竞态用。
    ensure_delay_ms: RwLock<u64>,
}

/// `ensure_contest_access` 的 stub 行为。
#[derive(Clone, Copy, PartialEq)]
enum AccessMode {
    /// 已可访问（Ok）
    Grant,
    /// 业务拒绝：密码错误（AppError::Contest —— 会被服务记入防重试记忆）
    WrongPassword,
    /// 瞬态失败（AppError::Network —— 不应记入记忆）
    Network,
    /// 会话失效（AppError::Auth —— 不应记入记忆，重新登录后应重试）
    Auth,
}

impl StubContestProvider {
    fn new(mode: StubMode) -> Self {
        Self {
            mode: RwLock::new(mode),
            calls: AtomicUsize::new(0),
            meta_calls: AtomicUsize::new(0),
            problems_calls: AtomicUsize::new(0),
            announcements: RwLock::new(sample_announcement_page().records),
            contest_auth: RwLock::new(0),
            problems_mode: RwLock::new(None),
            registered: AtomicBool::new(false),
            access_mode: RwLock::new(AccessMode::Grant),
            access_calls: AtomicUsize::new(0),
            access_credential: RwLock::new(None),
            ensure_delay_ms: RwLock::new(0),
        }
    }

    fn set_mode(&self, mode: StubMode) {
        *self.mode.write().unwrap() = mode;
    }

    /// 覆盖 `list_contest_problems` 的独立模式（`None` 恢复跟随全局）。
    fn set_problems_mode(&self, mode: Option<StubMode>) {
        *self.problems_mode.write().unwrap() = mode;
    }

    /// 设置 `get_contest` 返回的比赛 auth（0=公开，1=私有）。
    fn set_auth(&self, auth: i32) {
        *self.contest_auth.write().unwrap() = auth;
    }

    /// 设置 `ensure_contest_access` 的行为。
    fn set_access_mode(&self, mode: AccessMode) {
        *self.access_mode.write().unwrap() = mode;
    }

    /// 设置 `ensure_contest_access` 的延迟（复现「清理发生在途」竞态）。
    fn set_ensure_delay_ms(&self, ms: u64) {
        *self.ensure_delay_ms.write().unwrap() = ms;
    }

    /// `ensure_contest_access` 调用次数
    fn access_call_count(&self) -> usize {
        self.access_calls.load(Ordering::SeqCst)
    }

    /// 最近一次进入调用收到的凭证
    fn last_access_credential(&self) -> Option<String> {
        self.access_credential.read().unwrap().clone()
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

/// auth 可变的 sample_contest（私有赛进入分支测试用）。
fn sample_contest_with_auth(auth: i32) -> Contest {
    let mut contest = sample_contest();
    contest.auth = auth;
    contest
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
            StubMode::Ok => Ok(sample_contest_with_auth(*self.contest_auth.read().unwrap())),
            m => Err(m.into_err("stub get_contest")),
        }
    }

    async fn list_contest_problems(&self, _contest_id: &str) -> AppResult<Vec<ContestProblem>> {
        self.problems_calls.fetch_add(1, Ordering::SeqCst);
        let mode = self
            .problems_mode
            .read()
            .unwrap()
            .unwrap_or_else(|| self.current_mode());
        match mode {
            StubMode::Ok => Ok(vec![sample_problem()]),
            // Contest 语义是「未报名/需凭证」：报名成功（registered 置位）后放行，
            // 与服务端事实一致 —— 进入流程成功后的重试必须能拿到题目
            StubMode::Contest if self.registered.load(Ordering::SeqCst) => {
                Ok(vec![sample_problem()])
            }
            // ContestAlways 无视报名状态：报名后题目仍失败（服务端状态未解锁的形态）
            StubMode::Contest | StubMode::ContestAlways => {
                Err(mode.into_err("stub list_contest_problems"))
            }
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

    async fn ensure_contest_access(
        &self,
        _contest_id: &str,
        credential: Option<&str>,
    ) -> AppResult<()> {
        self.access_calls.fetch_add(1, Ordering::SeqCst);
        *self.access_credential.write().unwrap() = credential.map(str::to_string);
        let delay_ms = *self.ensure_delay_ms.read().unwrap();
        if delay_ms > 0 {
            // 竞态用例：让「清理」能落在本次调用在途期间
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }
        match *self.access_mode.read().unwrap() {
            AccessMode::Grant => {
                // 模拟服务端：报名成功解锁私有赛题目列表
                self.registered.store(true, Ordering::SeqCst);
                Ok(())
            }
            AccessMode::WrongPassword => Err(AppError::Contest("密码错误".to_string())),
            AccessMode::Network => Err(AppError::Network(
                "stub ensure_contest_access: connection reset".to_string(),
            )),
            AccessMode::Auth => Err(AppError::Auth(
                "stub ensure_contest_access: HTTP 401".into(),
            )),
        }
    }

    /// stub 的 Contest/ContestAlways 消息全部代表「需凭证进入」类失败，
    /// 故按变体判定即可（真实消息级分类由 HOJ 适配器测试锁定）。
    fn is_credential_rejection(&self, error: &AppError) -> bool {
        matches!(error, AppError::Contest(_))
    }

    /// stub 的 Contest 错误全部代表「注册被拒」（确定性结果，进结论记忆）；
    /// Network/Auth 错误走默认的 Other（退避，不进结论记忆）——与
    /// AccessMode 语义一一对应
    fn classify_access_rejection(&self, error: &AppError) -> AccessRejectionKind {
        match error {
            AppError::Contest(_) => AccessRejectionKind::Rejected,
            _ => AccessRejectionKind::Other,
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

/// 同 `make_service`，但把事件总线也交出来（断言事件发布契约）。
fn make_service_with_bus(
    mode: StubMode,
) -> (
    ContestService,
    Arc<StubContestProvider>,
    Arc<CoreEventBus>,
    TempDir,
) {
    let dir = unique_temp_dir("contest-events");
    let (service, provider, bus) = build_service_in(&dir, mode);
    (service, provider, bus, dir)
}

fn build_service_in(
    dir: &TempDir,
    mode: StubMode,
) -> (ContestService, Arc<StubContestProvider>, Arc<CoreEventBus>) {
    let provider = Arc::new(StubContestProvider::new(mode));
    let access = Arc::new(crate::core::access::AccessRevision::default());
    let registry: Arc<dyn ProviderRegistry> =
        Arc::new(ProviderRegistryImpl::new(OjId::new("HOJ"), access.clone()));
    registry.register(
        OjId::new("HOJ"),
        ProviderSet {
            contest: Some(Arc::clone(&provider) as Arc<dyn ContestProvider>),
            ..Default::default()
        },
    );
    let bus = Arc::new(CoreEventBus::new());
    let service = ContestService::new(
        registry,
        Arc::clone(&bus),
        Arc::new(Storage::new(dir.to_path_buf())),
        access,
    );
    (service, provider, bus)
}

/// 订阅总线（事件断言用）。
///
/// 用 receiver 同步取事件（`try_recv`）而不是后台线程收集：被测调用返回时事件
/// 已在 broadcast 缓冲里，因此**「不该发事件」的负向断言也是确定的** ——
/// 后台线程收集会让负向断言变成竞态（事件可能只是还没被搬进 Vec）。
fn subscribe(bus: &Arc<CoreEventBus>) -> tokio::sync::broadcast::Receiver<CoreEvent> {
    bus.subscribe()
}

/// 取出当前已缓冲的「新公告」事件载荷（非阻塞）。
fn drain_new_announcement_ids(
    rx: &mut tokio::sync::broadcast::Receiver<CoreEvent>,
) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let CoreEvent::AnnouncementChanged { new_ids, .. } = event {
            out.push(new_ids);
        }
    }
    out
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
    let err =
        block_on(service.get_rank("1011", &RankQuery::default())).expect_err("token 过期应报错");
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
    let err =
        block_on(service.get_rank("1011", &RankQuery::default())).expect_err("网络异常应报错");
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
    assert!(matches!(err, AppError::Auth(_)), "实际 {:?}", err);
}

#[test]
fn load_contest_with_problems_preserves_auth_variant() {
    // 进场链路：外壳 loadContest 走的就是这个方法
    let (service, _stub, _dir) = make_service(StubMode::Auth);
    let err =
        block_on(service.load_contest_with_problems("1011", None)).expect_err("token 过期应报错");
    assert!(
        matches!(err, AppError::Auth(_)),
        "进场时 401 必须触发会话守卫，实际 {:?}",
        err
    );
}

#[test]
fn list_announcements_preserves_auth_variant() {
    let (service, _stub, _dir) = make_service(StubMode::Auth);
    let err = block_on(service.list_announcements("1011", 1, 50)).expect_err("token 过期应报错");
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
    let err = block_on(service.list_announcements("1011", 1, 50)).expect_err("网络异常应报错");
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
    let page = block_on(service.get_rank("1011", &RankQuery::default())).expect("成功路径不应报错");
    assert_eq!(page.records.len(), 1);
    assert_eq!(page.records[0].uid, "u1");
}

#[test]
fn load_contest_with_problems_returns_bundle_and_selects() {
    let (service, _stub, _dir) = make_service(StubMode::Ok);
    let bundle =
        block_on(service.load_contest_with_problems("1011", None)).expect("成功路径不应报错");
    assert_eq!(bundle.contest.id, "1011");
    assert_eq!(bundle.problems.len(), 1);
    assert_eq!(
        service.current_contest_id(),
        Some("1011".to_string()),
        "加载后应自动选中该比赛"
    );
}

// ── 私有赛进入（auth==1：错误驱动 + 凭证报名 + 防轮询重复尝试记忆）──
//
// 题目列表先行：已报名用户、比赛管理员、开放赛外榜单（HOJ 不校验报名的用户）
// 零额外请求；仅当题目列表以 Contest 变体失败（适配器契约）时才走进入流程并
// 重试一次。记忆防 30s 轮询重复提交报名：成败皆记、失败项带保留期
// （见 AccessAttemptMemo），网络/会话类失败不记（瞬态或有恢复路径）。

#[test]
fn public_contest_never_triggers_access() {
    let (service, stub, _dir) = make_service(StubMode::Ok);
    block_on(service.load_contest_with_problems("1011", Some("pw"))).expect("公开赛不应报错");
    assert_eq!(
        stub.access_call_count(),
        0,
        "公开赛（auth=0）不应发起进入校验"
    );
}

#[test]
fn private_contest_with_accessible_problems_never_triggers_access_flow() {
    // 错误驱动的关键不变量：题目列表能成功（已报名/管理员/开放赛外榜单）
    // 时进入流程不得被触发 —— 否则这三类用户会被密码错误页误挡。
    // password=None 使提交侧 best-effort 兜底也不参与（它由
    // open_rank_private_contest_backfills_registration_best_effort 单独锁定）
    let (service, stub, _dir) = make_service(StubMode::Ok);
    stub.set_auth(1);
    block_on(service.load_contest_with_problems("1011", None)).expect("题目可拉的私有赛加载应成功");
    assert_eq!(
        stub.access_call_count(),
        0,
        "题目列表成功且无密码时不得发起任何进入请求"
    );
}

#[test]
fn open_rank_private_contest_backfills_registration_best_effort() {
    // 提交侧兜底：题目可拉（openRank 私有赛可见不可交）+ 配置了密码 →
    // **真后台**报名（tokio::spawn，不阻塞加载返回），报名成功记忆生效
    let (service, stub, _dir) = make_service(StubMode::Ok);
    stub.set_auth(1);
    block_on(async {
        service
            .load_contest_with_problems("1011", Some("pw"))
            .await
            .expect("加载应成功");
        // 兜底已 spawn：current_thread 上让出调度直到后台任务完成
        let mut waited = 0;
        while stub.access_call_count() < 1 && waited < 10_000 {
            tokio::task::yield_now().await;
            waited += 1;
        }
    });
    assert_eq!(
        stub.access_call_count(),
        1,
        "题目成功且配置了密码时应后台 best-effort 报名（提交通道对 auth==1 始终要求报名）"
    );
    assert_eq!(stub.last_access_credential(), Some("pw".to_string()));
    // 「成功记忆命中不重复请求」**不在本用例断言**：后台命中 memo 时无 provider
    // 调用可观察，yield 次数也不是完成信号（任务未被调度到则变异不红）。
    // 该不变量由 success_memo_hits_when_problems_still_fail 在错误驱动路径
    // （同步 await、可靠等待）锁定——后台兜底与其共用同一 core 实现。
}

#[test]
fn best_effort_backfill_failure_does_not_break_loading() {
    // 报名失败（密码错误）只留日志：查看侧不得被提交侧兜底挡住。
    // 同一 block_on 内等后台任务真正执行失败路径（provider 被调一次）——
    // 否则 runtime 关闭会丢弃未跑完的任务，「失败路径被执行」只是结构推断
    let (service, stub, _dir) = make_service(StubMode::Ok);
    stub.set_auth(1);
    stub.set_access_mode(AccessMode::WrongPassword);
    block_on(async {
        service
            .load_contest_with_problems("1011", Some("pw"))
            .await
            .expect("报名失败不影响加载");
        let mut waited = 0;
        while stub.access_call_count() < 1 && waited < 10_000 {
            tokio::task::yield_now().await;
            waited += 1;
        }
    });
    assert_eq!(
        stub.access_call_count(),
        1,
        "后台失败路径确实执行（provider 被调一次，错误按 debug 级留痕）"
    );
}

#[test]
fn private_contest_problems_failure_triggers_access_and_retries() {
    let (service, stub, _dir) = make_service(StubMode::Ok);
    stub.set_auth(1);
    stub.set_problems_mode(Some(StubMode::Contest));
    stub.set_access_mode(AccessMode::Grant);

    block_on(service.load_contest_with_problems("1011", Some("pw")))
        .expect("进入成功后重试题目列表应成功");
    assert_eq!(stub.access_call_count(), 1);
    assert_eq!(
        stub.last_access_credential(),
        Some("pw".to_string()),
        "配置里的比赛密码必须透传到 provider"
    );
    assert_eq!(stub.problems_call_count(), 2, "进入成功后必须重试题目列表");
}

#[test]
fn public_contest_problems_failure_is_not_rerouted_to_access() {
    let (service, stub, _dir) = make_service(StubMode::Ok);
    stub.set_problems_mode(Some(StubMode::Contest)); // auth 仍为 0

    let err = block_on(service.load_contest_with_problems("1011", Some("pw")))
        .expect_err("公开赛题目失败应原样报错");
    assert!(
        matches!(err, AppError::Contest(ref m) if m.contains("获取比赛题目列表失败")),
        "公开赛题目失败保留 Contest 变体 + 环节名，实际 {:?}",
        err
    );
    assert_eq!(stub.access_call_count(), 0, "公开赛不得走进入流程");
}

#[test]
fn private_contest_access_failure_surfaces_access_error() {
    // 进入失败时上抛进入的引导错误（可操作），而不是题目接口的裸服务端报错
    let (service, stub, _dir) = make_service(StubMode::Ok);
    stub.set_auth(1);
    stub.set_problems_mode(Some(StubMode::Contest));
    stub.set_access_mode(AccessMode::WrongPassword);

    let err = block_on(service.load_contest_with_problems("1011", Some("bad")))
        .expect_err("密码错误应报错");
    assert!(
        matches!(err, AppError::Contest(ref m) if m.contains("密码错误") && m.contains("私有赛进入校验失败")),
        "应报进入校验的引导错误，实际 {:?}",
        err
    );
    assert_eq!(stub.problems_call_count(), 1, "进入失败后不得重试题目列表");
}

#[test]
fn private_contest_business_failure_is_memoized() {
    let (service, stub, _dir) = make_service(StubMode::Ok);
    stub.set_auth(1);
    stub.set_problems_mode(Some(StubMode::Contest));
    stub.set_access_mode(AccessMode::WrongPassword);

    let err1 = block_on(service.load_contest_with_problems("1011", Some("bad")))
        .expect_err("密码错误应报错");
    assert!(
        matches!(err1, AppError::Contest(ref m) if m.contains("密码错误") && m.contains("私有赛进入校验失败")),
        "业务失败保留 Contest 变体并补环节名，实际 {:?}",
        err1
    );

    // 第二次加载（模拟 30s 轮询重试）：命中记忆，不再发起进入请求
    let err2 = block_on(service.load_contest_with_problems("1011", Some("bad")))
        .expect_err("记忆命中的失败应继续报错");
    assert_eq!(
        err1.user_message(),
        err2.user_message(),
        "轮询重试应复用首次的引导消息"
    );
    assert_eq!(
        stub.access_call_count(),
        1,
        "同（OJ, 比赛, 密码）组合在保留期内不得重复提交"
    );
    assert_eq!(stub.problems_call_count(), 2, "每次加载题目列表各失败一次");
}

#[test]
fn private_contest_network_failure_backs_off_within_interval() {
    // Network 类失败结论不升级，条目保持 Attempted：最小重发间隔内拦住同 key
    // 重发（在途防重 + 退避——OJ 故障时不再每 30s 轮询都打请求）；错误驱动
    // 路径被拦时上抛原始题目错误（本轮未发起进入，下周期自愈）
    let (service, stub, _dir) = make_service(StubMode::Ok);
    stub.set_auth(1);
    stub.set_problems_mode(Some(StubMode::Contest));
    stub.set_access_mode(AccessMode::Network);

    let err1 = block_on(service.load_contest_with_problems("1011", Some("pw")))
        .expect_err("网络失败应报错");
    assert!(
        matches!(err1, AppError::Network(_)),
        "网络类失败不应被改写变体（否则会误踢会话/误引导），实际 {:?}",
        err1
    );

    let err2 = block_on(service.load_contest_with_problems("1011", Some("pw")))
        .expect_err("退避窗口内的加载仍应报错");
    assert!(
        matches!(err2, AppError::Contest(ref m) if m.contains("获取比赛题目列表失败")),
        "被拦时上抛原始题目错误（本轮未发起进入），实际 {:?}",
        err2
    );
    assert_eq!(
        stub.access_call_count(),
        1,
        "退避窗口内不得重复发起进入请求"
    );
}

#[test]
fn private_contest_auth_failure_is_not_memoized() {
    // 会话失效的结论记忆不收（有恢复路径：重新登录，login 命令清空记忆）；
    // 但 Attempted 条目仍在最小重发间隔内——第二次加载被拦（上抛原始题目
    // 错误），退避过期后才会重试
    let (service, stub, _dir) = make_service(StubMode::Ok);
    stub.set_auth(1);
    stub.set_problems_mode(Some(StubMode::Contest));
    stub.set_access_mode(AccessMode::Auth);

    let err1 = block_on(service.load_contest_with_problems("1011", Some("pw")))
        .expect_err("会话失效应报错");
    assert!(
        matches!(err1, AppError::Auth(_)),
        "Auth 失败必须保持 Auth 变体（首次失败触发会话守卫），实际 {:?}",
        err1
    );

    let err2 = block_on(service.load_contest_with_problems("1011", Some("pw")))
        .expect_err("退避窗口内的加载仍应报错");
    assert!(
        matches!(err2, AppError::Contest(ref m) if m.contains("获取比赛题目列表失败")),
        "退避窗口内被拦，上抛原始题目错误，实际 {:?}",
        err2
    );
    assert_eq!(
        stub.access_call_count(),
        1,
        "退避窗口内不得重复发起进入请求"
    );
}

#[test]
fn password_change_retries_after_memoized_failure() {
    let (service, stub, _dir) = make_service(StubMode::Ok);
    stub.set_auth(1);
    stub.set_problems_mode(Some(StubMode::Contest));
    stub.set_access_mode(AccessMode::WrongPassword);

    block_on(service.load_contest_with_problems("1011", Some("bad")))
        .expect_err("首次密码错误应报错");
    block_on(service.load_contest_with_problems("1011", Some("bad")))
        .expect_err("重试同密码仍报错");
    assert_eq!(stub.access_call_count(), 1, "同密码组合在保留期内命中记忆");

    // 用户在设置页更正密码 → 新 key → 重新发起（允许一次新尝试）；
    // 报名成功解锁题目列表（registered），重试拿到题目、本次加载成功
    stub.set_access_mode(AccessMode::Grant);
    block_on(service.load_contest_with_problems("1011", Some("good"))).expect("更正密码后应成功");
    assert_eq!(stub.last_access_credential(), Some("good".to_string()));
    // 成功记忆的服务级路径由 success_memo_hits_when_problems_still_fail 锁定
    //（本用例报名后题目列表直接放行，走不到记忆）
}

#[test]
fn success_memo_hits_when_problems_still_fail() {
    // 服务级成功记忆路径：报名成功但题目仍失败（ContestAlways 无视 registered）。
    // 第二次加载必须命中成功记忆、不再触达 ensure_contest_access ——
    // 删掉成功记忆分支本用例会红（变异验证）
    let (service, stub, _dir) = make_service(StubMode::Ok);
    stub.set_auth(1);
    stub.set_problems_mode(Some(StubMode::ContestAlways));
    stub.set_access_mode(AccessMode::Grant);

    block_on(service.load_contest_with_problems("1011", Some("pw")))
        .expect_err("报名成功但题目仍失败，本次加载应上抛题目错误");
    assert_eq!(stub.access_call_count(), 1);

    block_on(service.load_contest_with_problems("1011", Some("pw")))
        .expect_err("第二次加载题目仍失败");
    assert_eq!(
        stub.access_call_count(),
        1,
        "成功记忆命中：不再触达 ensure_contest_access"
    );
    assert_eq!(
        stub.problems_call_count(),
        4,
        "两次加载各失败 1 次 + 各重试 1 次"
    );
}

#[test]
fn oj_switch_clears_access_attempts() {
    let (service, stub, _dir) = make_service(StubMode::Ok);
    stub.set_auth(1);
    stub.set_problems_mode(Some(StubMode::Contest));
    stub.set_access_mode(AccessMode::WrongPassword);
    block_on(service.load_contest_with_problems("1011", Some("bad")))
        .expect_err("首次密码错误应报错");

    service.on_oj_switched();

    stub.set_access_mode(AccessMode::Grant);
    block_on(service.load_contest_with_problems("1011", Some("bad")))
        .expect("OJ 切换后记忆已清空，应重新发起进入");
    assert_eq!(stub.access_call_count(), 2, "OJ 切换必须清空进入尝试记录");
}

#[test]
fn clear_access_attempts_forgets_recorded_outcomes() {
    // 登出/成功登录/重置客户端走的清理入口：失败记忆失效后，同组合必须
    // 重新触达 provider（换账号场景：新账号本可访问，不得被旧账号的失败卡死）
    let (service, stub, _dir) = make_service(StubMode::Ok);
    stub.set_auth(1);
    stub.set_problems_mode(Some(StubMode::Contest));
    stub.set_access_mode(AccessMode::WrongPassword);

    block_on(service.load_contest_with_problems("1011", Some("pw")))
        .expect_err("首次密码错误应报错");
    block_on(service.load_contest_with_problems("1011", Some("pw"))).expect_err("保留期内命中记忆");
    assert_eq!(stub.access_call_count(), 1);

    service.clear_access_attempts();

    stub.set_access_mode(AccessMode::Grant);
    block_on(service.load_contest_with_problems("1011", Some("pw")))
        .expect("清空后应重新发起进入并成功");
    assert_eq!(
        stub.access_call_count(),
        2,
        "清空后同组合必须重新触达 provider"
    );
}

#[test]
fn concurrent_loads_do_not_double_register() {
    // 同临界区 lookup+begin 的并发行为：多线程 runtime 下两个加载同时进入
    // core，先到者在临界区内记录 Attempted、后到者必然看到 Busy → Skipped，
    // 不触达 provider（互斥锁保证该断言确定成立）。若把「查」与「记」拆回
    // 两次加锁，双线程交错可双双 Miss → 双双发起注册（变异在此构造下
    // 大概率变红——竞态本质是时序相关的，故本用例是压力测试而非确定性
    // 变异验证；结构性正确由「单临界区」实现保证）
    let (service, stub, _dir) = make_service(StubMode::Ok);
    let service = Arc::new(service);
    stub.set_auth(1);
    stub.set_problems_mode(Some(StubMode::ContestAlways));
    stub.set_access_mode(AccessMode::Grant);
    stub.set_ensure_delay_ms(150);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("创建多线程测试 runtime 失败");
    rt.block_on(async {
        let s1 = Arc::clone(&service);
        let s2 = Arc::clone(&service);
        let l1 =
            tokio::spawn(async move { s1.load_contest_with_problems("1011", Some("pw")).await });
        let l2 =
            tokio::spawn(async move { s2.load_contest_with_problems("1011", Some("pw")).await });
        for r in [l1, l2] {
            assert!(
                r.await.expect("任务不应 panic").is_err(),
                "题目恒失败，两次加载都应报错"
            );
        }
    });

    assert_eq!(
        stub.access_call_count(),
        1,
        "并发加载不得双双发起注册（第二个加载必然被 Attempted 拦为 Busy）"
    );
}

#[test]
fn in_flight_access_attempt_is_not_written_back_after_clear() {
    // clear_access_attempts 是时点清理：logout/login 期间仍在途的旧会话加载
    // 若在清理后才完成，不得把旧账号的尝试结果写回记忆（access_epoch 守卫）。
    // 用带延迟的 stub 复现竞态：清理落在 ensure 调用在途期间。
    let (service, stub, _dir) = make_service(StubMode::Ok);
    let service = Arc::new(service);
    stub.set_auth(1);
    stub.set_problems_mode(Some(StubMode::ContestAlways));
    stub.set_access_mode(AccessMode::Grant);
    stub.set_ensure_delay_ms(300);

    block_on(async {
        let svc = Arc::clone(&service);
        let load =
            tokio::spawn(async move { svc.load_contest_with_problems("1011", Some("pw")).await });
        // 让在途 load 进入 ensure 调用（延迟中），期间执行清理
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        service.clear_access_attempts();
        let result = load.await.expect("后台任务不应 panic");
        assert!(result.is_err(), "题目恒失败，在途加载应报错");
    });

    // 在途写回被纪元拦截：下一次加载必须重新触达 provider
    //（若写回泄漏，成功记忆命中会让本断言得 1 而变红）
    block_on(service.load_contest_with_problems("1011", Some("pw"))).expect_err("题目仍失败");
    assert_eq!(
        stub.access_call_count(),
        2,
        "在途写回必须被纪元丢弃，清理后的加载重新发起进入"
    );
}

#[test]
fn access_memo_rejected_expires_after_retention() {
    // 「同一组合的尝试结果不变」并不严格成立（服务端可能撤销报名）：Rejected
    // 记忆带保留期，过期即视为从未尝试
    let mut memo = AccessAttemptMemo::new();
    let key: AccessAttemptKey = ("HOJ".into(), "1011".into(), Some(123));
    let t0 = Instant::now();
    memo.record_rejected(key.clone(), "密码错误".to_string(), t0);

    assert!(
        matches!(
            memo.lookup(&key, t0 + Duration::from_secs(1)),
            AccessMemoLookup::Failure(ref m) if m == "密码错误"
        ),
        "保留期内复用首次消息"
    );
    assert!(
        matches!(
            memo.lookup(&key, t0 + ACCESS_MEMO_RETENTION - Duration::from_secs(1)),
            AccessMemoLookup::Failure(_)
        ),
        "保留期最后一秒内仍命中"
    );
    assert!(
        matches!(
            memo.lookup(&key, t0 + ACCESS_MEMO_RETENTION),
            AccessMemoLookup::Miss
        ),
        "保留期一过即视为未尝试"
    );
    assert!(
        matches!(
            memo.lookup(&key, t0 + ACCESS_MEMO_RETENTION),
            AccessMemoLookup::Miss
        ),
        "过期条目已被顺手清除，不会反复判过期"
    );
}

#[test]
fn access_memo_success_expires_for_recheck() {
    // 成功不能永久可信：服务端可能撤销报名。成功条目带复核保留期——过期后
    // 视为未尝试，下次加载重新查 access（撤销后 ≤ 保留期自愈，已报名用户
    // 的代价是每保留期一次权限查询）
    let mut memo = AccessAttemptMemo::new();
    let key: AccessAttemptKey = ("HOJ".into(), "1011".into(), Some(123));
    let t0 = Instant::now();
    memo.record_success(key.clone(), t0);
    assert!(matches!(
        memo.lookup(&key, t0 + ACCESS_MEMO_RETENTION - Duration::from_secs(1)),
        AccessMemoLookup::Success
    ));
    assert!(
        matches!(
            memo.lookup(&key, t0 + ACCESS_MEMO_RETENTION),
            AccessMemoLookup::Miss
        ),
        "保留期一过即重新校验"
    );
}

#[test]
fn access_memo_attempted_blocks_within_interval_then_expires() {
    // Attempted 条目（请求发起前记录）：最小重发间隔内 Busy 拦住同 key 重发
    //（在途防重 + 网络退避），过期放行
    let mut memo = AccessAttemptMemo::new();
    let key: AccessAttemptKey = ("HOJ".into(), "1011".into(), Some(123));
    let t0 = Instant::now();
    memo.record_attempted(key.clone(), t0);

    assert!(
        matches!(
            memo.lookup(&key, t0 + Duration::from_secs(1)),
            AccessMemoLookup::Busy
        ),
        "最小重发间隔内拦住重发（注册在途/网络退避）"
    );
    assert!(
        matches!(
            memo.lookup(&key, t0 + ACCESS_RETRY_MIN_INTERVAL),
            AccessMemoLookup::Miss
        ),
        "间隔一过即放行（重试/下周期自愈）"
    );
}

#[test]
fn access_memo_record_sweeps_expired_entries() {
    // 过期条目只在同 key 再查时被发现的话，改过多次密码的用户会让旧条目
    // 无限累积——每次写回顺带清扫全表
    let mut memo = AccessAttemptMemo::new();
    let t0 = Instant::now();
    let pw1: AccessAttemptKey = ("HOJ".into(), "1011".into(), Some(1));
    let pw2: AccessAttemptKey = ("HOJ".into(), "1011".into(), Some(2));
    let pw3: AccessAttemptKey = ("HOJ".into(), "1011".into(), Some(3));
    memo.record_rejected(pw1.clone(), "密码错误 v1".to_string(), t0);
    memo.record_rejected(pw2.clone(), "密码错误 v2".to_string(), t0);

    // v2 过期清扫发生在 t0 + RETENTION 的任意一次写回：v1（同批过期）一并清除，
    // 新条目（v3）保留
    memo.record_rejected(
        pw3.clone(),
        "密码错误 v3".to_string(),
        t0 + ACCESS_MEMO_RETENTION,
    );
    assert!(
        matches!(
            memo.lookup(&pw1, t0 + ACCESS_MEMO_RETENTION),
            AccessMemoLookup::Miss
        ),
        "同批过期条目被写回时顺带清扫"
    );
    assert!(matches!(
        memo.lookup(&pw2, t0 + ACCESS_MEMO_RETENTION),
        AccessMemoLookup::Miss
    ));
    assert!(
        matches!(
            memo.lookup(&pw3, t0 + ACCESS_MEMO_RETENTION),
            AccessMemoLookup::Failure(_)
        ),
        "新写入的条目不受清扫影响"
    );
}

#[test]
fn credential_fingerprint_distinguishes_passwords() {
    // 密码只以指纹参与键：不同密码不同指纹（同密码同指纹），明文不落记忆表
    let none = credential_fingerprint(&None);
    let a = credential_fingerprint(&Some("pw-a".to_string()));
    let a2 = credential_fingerprint(&Some("pw-a".to_string()));
    let b = credential_fingerprint(&Some("pw-b".to_string()));
    assert!(none.is_none());
    assert_eq!(a, a2, "同密码同指纹");
    assert_ne!(a, b, "不同密码不同指纹");
}

#[test]
fn access_memo_missing_key_is_miss() {
    let mut memo = AccessAttemptMemo::new();
    let key: AccessAttemptKey = ("HOJ".into(), "1011".into(), None);
    assert!(matches!(
        memo.lookup(&key, Instant::now()),
        AccessMemoLookup::Miss
    ));
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
// 走 CoreEventBus，由 main.rs 的事件桥转发到 webview。以下锁定三个不变量。

#[test]
fn first_announcement_fetch_establishes_baseline_without_event() {
    // 首次拉取没有基线可比：发事件等于一开机就给每位选手亮红点
    let (service, _stub, bus, _dir) = make_service_with_bus(StubMode::Ok);
    let mut rx = subscribe(&bus);

    block_on(service.list_announcements("1011", 1, 50)).expect("首次拉取应成功");

    assert!(
        drain_new_announcement_ids(&mut rx).is_empty(),
        "首次拉取不应发布新公告事件"
    );
}

#[test]
fn new_announcement_publishes_event_once() {
    let (service, stub, bus, _dir) = make_service_with_bus(StubMode::Ok);
    let mut rx = subscribe(&bus);

    // 建基线
    block_on(service.list_announcements("1011", 1, 50)).expect("首次拉取应成功");

    // 裁判组发布 9002
    stub.set_announcements(&["9001", "9002"]);
    block_on(service.list_announcements("1011", 1, 50)).expect("二次拉取应成功");

    assert_eq!(
        drain_new_announcement_ids(&mut rx),
        vec![vec!["9002".to_string()]],
        "应且仅应发布一次新公告事件，且只带新增 ID"
    );

    // 再拉一次（列表不变）：不得重复发事件，否则红点会被反复点亮
    block_on(service.list_announcements("1011", 1, 50)).expect("三次拉取应成功");
    assert!(
        drain_new_announcement_ids(&mut rx).is_empty(),
        "同一批公告不得重复发事件"
    );
}

#[test]
fn announcement_baseline_is_isolated_per_contest() {
    // 基线按比赛隔离：只看过 1011 的情况下首次看 1012 不应把 1012 的既有公告
    // 当成「新公告」（同一进程内切换比赛是常态）
    let (service, stub, bus, _dir) = make_service_with_bus(StubMode::Ok);
    let mut rx = subscribe(&bus);

    block_on(service.list_announcements("1011", 1, 50)).expect("1011 首次拉取应成功");
    block_on(service.list_announcements("1012", 1, 50)).expect("1012 首次拉取应成功");
    assert!(
        drain_new_announcement_ids(&mut rx).is_empty(),
        "各自首次拉取都不应发事件"
    );

    // 1012 新增一条：只影响 1012
    stub.set_announcements(&["9001", "9002"]);
    block_on(service.list_announcements("1012", 1, 50)).expect("1012 二次拉取应成功");

    let published = drain_new_announcement_ids(&mut rx);
    assert_eq!(published.len(), 1, "只有 1012 应发事件");
    assert_eq!(published[0], vec!["9002".to_string()]);
}

#[test]
fn failed_fetch_keeps_baseline_so_next_success_still_reports() {
    // 拉取失败时不得推进基线：否则失败期间发布的公告会被永久漏报
    let (service, stub, bus, _dir) = make_service_with_bus(StubMode::Ok);
    let mut rx = subscribe(&bus);

    block_on(service.list_announcements("1011", 1, 50)).expect("首次拉取应成功");

    stub.set_mode(StubMode::Network);
    let _ = block_on(service.list_announcements("1011", 1, 50));
    stub.set_mode(StubMode::Ok);

    stub.set_announcements(&["9001", "9002"]);
    block_on(service.list_announcements("1011", 1, 50)).expect("恢复后拉取应成功");

    assert_eq!(
        drain_new_announcement_ids(&mut rx),
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

    assert_eq!(
        stub.meta_call_count(),
        1,
        "元信息应命中缓存，不再请求 Provider"
    );
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

    let err =
        block_on(service.load_contest_with_problems("1011", None)).expect_err("token 过期应报错");
    assert!(
        matches!(err, AppError::Auth(_)),
        "变体必须保留，实际 {:?}",
        err
    );

    stub.set_mode(StubMode::Ok);
    block_on(service.load_contest_with_problems("1011", None)).expect("恢复后应成功");
    assert_eq!(
        stub.meta_call_count(),
        2,
        "错误不得入缓存，恢复后必须重新请求"
    );
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

// ── OJ 切换：OJ 域缓存失效（显式调用，不依赖事件投递）──

#[test]
fn contest_meta_cache_key_carries_oj_scope_so_cross_oj_never_hits() {
    // 「延迟清理磁盘缓存」安全的前提：键带 OJ 维度 → 跨 OJ 结构上不可能命中
    let dir = TempDir::named("hinina-test-contest-oj-scope");
    let provider = Arc::new(StubContestProvider::new(StubMode::Ok));
    let access = Arc::new(crate::core::access::AccessRevision::default());
    let registry: Arc<dyn ProviderRegistry> =
        Arc::new(ProviderRegistryImpl::new(OjId::new("HOJ"), access.clone()));
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
        Arc::new(CoreEventBus::new()),
        Arc::new(Storage::new(dir.to_path_buf())),
        access,
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

/// `on_oj_switched()` 显式清理 OJ 域缓存：内存段同步完成、磁盘段同步/后台完成。
///
/// 本用例是**同步**上下文（无 tokio runtime），因此走 `on_oj_switched` 的同步兜底分支 ——
/// 返回即已清完，断言是确定的。异步分支由下一个用例覆盖。
#[test]
fn on_oj_switched_clears_contest_scoped_caches() {
    let dir = TempDir::named("hinina-test-contest-oj-switch");
    let bus = Arc::new(CoreEventBus::new());
    let provider = Arc::new(StubContestProvider::new(StubMode::Ok));
    let access = Arc::new(crate::core::access::AccessRevision::default());
    let registry: Arc<dyn ProviderRegistry> =
        Arc::new(ProviderRegistryImpl::new(OjId::new("HOJ"), access.clone()));
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
        access,
    );

    // 预置三层缓存：列表（内存）+ 元信息（内存 + **磁盘**）。
    // 磁盘条目必须真实落盘 —— 否则「目录不存在」的断言恒真，等于零覆盖
    *service.cache.write().unwrap() = Some(ContestCache {
        contests: vec![],
        fetched_at: Instant::now(),
    });
    block_on(service.load_contest_meta("7")).expect("预置元信息失败");
    let disk_entry = dir
        .join("cache")
        .join("contest_meta")
        .join("HOJ")
        .join("7.json");
    assert!(disk_entry.exists(), "预置失败：元信息磁盘缓存未落盘");
    assert!(
        !service.meta_cache.is_empty(),
        "预置失败：元信息内存缓存为空"
    );

    // 显式调用（由 `switch_oj` 命令触发）：**不经过事件投递**
    service.on_oj_switched();

    assert!(service.cache.read().unwrap().is_none(), "列表缓存应被清空");
    assert!(service.meta_cache.is_empty(), "元信息内存缓存应被清空");
    assert!(
        !dir.join("cache/contest_meta").exists(),
        "磁盘元信息缓存应被清理（同步上下文下立即完成）"
    );
}

/// 有 tokio 上下文时，磁盘段走后台任务（空间回收不阻塞 `switch_oj` 返回）。
#[tokio::test]
async fn on_oj_switched_disk_cleanup_runs_on_runtime() {
    let dir = TempDir::named("hinina-test-contest-oj-switch-async");
    let provider = Arc::new(StubContestProvider::new(StubMode::Ok));
    let access = Arc::new(crate::core::access::AccessRevision::default());
    let registry: Arc<dyn ProviderRegistry> =
        Arc::new(ProviderRegistryImpl::new(OjId::new("HOJ"), access.clone()));
    registry.register(
        OjId::new("HOJ"),
        ProviderSet {
            contest: Some(Arc::clone(&provider) as Arc<dyn ContestProvider>),
            ..Default::default()
        },
    );
    let service = ContestService::new(
        registry,
        Arc::new(CoreEventBus::new()),
        Arc::new(Storage::new(dir.to_path_buf())),
        access,
    );

    // 手工预置磁盘缓存（不经过网络）：目录与文件都真实存在
    let meta_dir = dir.join("cache").join("contest_meta").join("HOJ");
    std::fs::create_dir_all(&meta_dir).expect("创建缓存目录失败");
    std::fs::write(meta_dir.join("7.json"), "{}").expect("写入缓存失败");

    service.on_oj_switched();

    // 后台任务完成前目录可能仍在：轮询等待（上限 5 秒）
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while dir.join("cache/contest_meta").exists() && std::time::Instant::now() < deadline {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(
        !dir.join("cache/contest_meta").exists(),
        "后台段应清理磁盘元信息缓存"
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
    assert!(
        !service.meta_cache.is_empty(),
        "预置失败：元信息内存缓存为空"
    );
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
    let mut rx = subscribe(&bus);

    block_on(service.list_announcements("1012", 1, 20)).expect("首次拉取公告失败");
    assert!(
        drain_new_announcement_ids(&mut rx).is_empty(),
        "首次拉取只建基线，不该发事件"
    );

    service.clear_caches();

    // 清空之后裁判组补发了一条公告
    provider.set_announcements(&["9001", "9002"]);
    block_on(service.list_announcements("1012", 1, 20)).expect("二次拉取公告失败");

    assert_eq!(
        drain_new_announcement_ids(&mut rx),
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
    let mut rx = subscribe(&bus);

    block_on(service.list_announcements("1012", 1, 20)).expect("首次拉取公告失败");
    assert!(drain_new_announcement_ids(&mut rx).is_empty());

    service.clear_announcement_baseline();

    provider.set_announcements(&["9001", "9002"]);
    block_on(service.list_announcements("1012", 1, 20)).expect("二次拉取公告失败");

    assert!(
        drain_new_announcement_ids(&mut rx).is_empty(),
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

// ── 事件发布契约（事实通知，不是命令）──

/// `select_contest` 发布 `ContestSelected`（供审计 / 插件 / 前端刷新）。
#[test]
fn select_contest_publishes_contest_selected() {
    let (service, _stub, bus, _dir) = make_service_with_bus(StubMode::Ok);
    let mut rx = subscribe(&bus);

    service.select_contest("1011").expect("选中比赛失败");

    assert_eq!(
        rx.try_recv().expect("应发布 ContestSelected"),
        CoreEvent::ContestSelected {
            contest_id: "1011".into()
        }
    );
}

/// `list_contests` **不发布任何事件**：比赛列表是查询结果，真实数据由 IPC 返回。
///
/// 旧实现发布 `ListLoaded { contests }`（全量克隆 + 伪装成事件），既浪费又制造
/// 「前端可以靠事件拿列表」的错误预期。本用例是该删除的回归防线。
#[test]
fn list_contests_publishes_nothing() {
    let (service, _stub, bus, _dir) = make_service_with_bus(StubMode::Ok);
    let mut rx = subscribe(&bus);

    let contests = block_on(service.list_contests(60)).expect("获取比赛列表失败");
    assert!(!contests.is_empty(), "前置条件：应有比赛数据");

    assert!(
        rx.try_recv().is_err(),
        "查询结果不得伪装成事件；真实数据只经 IPC 返回值传递"
    );
}
