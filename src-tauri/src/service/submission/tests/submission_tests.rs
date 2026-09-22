use super::*;

use crate::test_support::{Guarded, TempDir};

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::core::entity::submission::{
    JudgeCase, JudgementResult, JudgementStatus, SubmissionCases, SubmissionDetail, SubmissionPage,
    SubmissionQuery, SubmissionRecord,
};
use crate::core::error::AppError;
use crate::core::event::core_event::CoreEvent;
use crate::core::event::core_event_bus::CoreEventBus;
use crate::core::provider::oj_id::OjId;
use crate::core::provider::registry::ProviderRegistry;
use crate::core::provider::registry::ProviderSet;
use crate::core::provider::submission::SubmissionProvider;
use crate::infra::provider_registry_impl::ProviderRegistryImpl;

/// 在当前线程创建独立 tokio runtime，避免嵌套 runtime panic。
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("创建测试 runtime 失败");
    rt.block_on(fut)
}

/// Stub 行为模式。
#[derive(Clone, Copy)]
enum StubMode {
    /// 提交与查询都成功（查询直接返回终态 Accepted）
    Ok,
    /// 一律返回 Auth 错误（模拟 token 过期）
    Auth,
    /// 一律返回 Network 错误（模拟断网）
    Network,
    /// 查询永远返回 Running（非终态，用于「不发事件、原样透传」用例）
    AlwaysRunning,
    /// 详情/测试点均返回**非终态**（Running），用于断言评测中结果不入缓存
    NonTerminalDetail,
}

struct StubSubmissionProvider {
    mode: StubMode,
    /// get_judgement 调用次数
    calls: Arc<AtomicUsize>,
    /// get_submission_detail 调用次数（断言详情缓存真的省掉请求）
    detail_calls: Arc<AtomicUsize>,
    /// get_submission_cases 调用次数
    cases_calls: Arc<AtomicUsize>,
}

fn accepted() -> JudgementResult {
    JudgementResult {
        status: JudgementStatus::Accepted,
        score: 100.0,
        time_ms: 15,
        memory_kb: 2048,
        error_message: None,
    }
}

fn running() -> JudgementResult {
    JudgementResult {
        status: JudgementStatus::Running,
        score: 0.0,
        time_ms: 0,
        memory_kb: 0,
        error_message: None,
    }
}

/// 每次调用一个独立存储根，避免并行测试相互覆盖快照。
///
/// 返回存储 + 目录守卫：目录活到用例结束、随 `Drop` 回收 —— 此前只在开始时
/// 清理，实测单次全量测试就会留下 700+ 个 `hinina-submission-service-test-*`。
fn temp_storage() -> (Arc<Storage>, TempDir) {
    let dir = TempDir::unique("hinina-submission-service-test");
    (Arc::new(Storage::new(dir.to_path_buf())), dir)
}

#[async_trait::async_trait]
impl SubmissionProvider for StubSubmissionProvider {
    async fn submit(
        &self,
        _contest_id: &str,
        _problem_id: &str,
        _display_id: &str,
        _language: &str,
        _source_code: &str,
    ) -> AppResult<String> {
        match self.mode {
            StubMode::Ok | StubMode::AlwaysRunning | StubMode::NonTerminalDetail => {
                Ok("submit-1".into())
            }
            StubMode::Auth => Err(AppError::Auth("stub: HTTP 401 Unauthorized".into())),
            StubMode::Network => Err(AppError::Network("stub: connection reset".into())),
        }
    }

    async fn get_judgement(&self, _submission_id: &str) -> AppResult<JudgementResult> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.mode {
            StubMode::Ok => Ok(accepted()),
            StubMode::Auth => Err(AppError::Auth("stub: HTTP 401 Unauthorized".into())),
            StubMode::Network => Err(AppError::Network("stub: connection reset".into())),
            StubMode::AlwaysRunning | StubMode::NonTerminalDetail => Ok(running()),
        }
    }

    async fn list_contest_submissions(
        &self,
        _query: &SubmissionQuery,
    ) -> AppResult<SubmissionPage> {
        match self.mode {
            StubMode::Ok | StubMode::AlwaysRunning | StubMode::NonTerminalDetail => {
                Ok(SubmissionPage {
                    records: vec![SubmissionRecord {
                        submit_id: "12345".into(),
                        pid: "1061".into(),
                        display_pid: "HOJ-1061".into(),
                        title: "A + B".into(),
                        display_id: "A".into(),
                        username: "alice".into(),
                        submit_time: 1_700_000_000,
                        status: JudgementStatus::Accepted,
                        time_ms: 15,
                        memory_kb: 2048,
                        score: None,
                        length: 256,
                        language: "C++".into(),
                    }],
                    total: 1,
                    size: 20,
                    current: 1,
                    pages: 1,
                })
            }
            StubMode::Auth => Err(AppError::Auth("stub: HTTP 401 Unauthorized".into())),
            StubMode::Network => Err(AppError::Network("stub: connection reset".into())),
        }
    }

    async fn get_submission_detail(&self, _submit_id: &str) -> AppResult<SubmissionDetail> {
        self.detail_calls.fetch_add(1, Ordering::SeqCst);
        match self.mode {
            StubMode::Ok | StubMode::AlwaysRunning | StubMode::NonTerminalDetail => {
                Ok(SubmissionDetail {
                    submit_id: "12345".into(),
                    pid: "1061".into(),
                    display_pid: "HOJ-1061".into(),
                    username: "alice".into(),
                    submit_time: 1_700_000_000,
                    status: if matches!(self.mode, StubMode::NonTerminalDetail) {
                        JudgementStatus::Running
                    } else {
                        JudgementStatus::CompilationError
                    },
                    time_ms: 0,
                    memory_kb: 0,
                    score: None,
                    length: 256,
                    language: "C++".into(),
                    code: "int main(){}".into(),
                    error_message: Some("expected ';'".into()),
                    judger: None,
                    oi_rank_score: None,
                })
            }
            StubMode::Auth => Err(AppError::Auth("stub: HTTP 401 Unauthorized".into())),
            StubMode::Network => Err(AppError::Network("stub: connection reset".into())),
        }
    }

    async fn get_submission_cases(&self, _submit_id: &str) -> AppResult<SubmissionCases> {
        self.cases_calls.fetch_add(1, Ordering::SeqCst);
        match self.mode {
            StubMode::Ok | StubMode::AlwaysRunning | StubMode::NonTerminalDetail => {
                Ok(SubmissionCases {
                    cases: vec![JudgeCase {
                        case_id: 1,
                        seq: 1,
                        status: JudgementStatus::Accepted,
                        time_ms: 10,
                        memory_kb: 1024,
                        score: None,
                        group_num: None,
                    }],
                    sub_tasks: Vec::new(),
                    mode: "default".into(),
                })
            }
            StubMode::Auth => Err(AppError::Auth("stub: HTTP 401 Unauthorized".into())),
            StubMode::Network => Err(AppError::Network("stub: connection reset".into())),
        }
    }
}

/// Stub 计数器句柄（断言缓存是否真的省掉请求）。
struct StubCounters {
    judgement: Arc<AtomicUsize>,
    detail: Arc<AtomicUsize>,
    cases: Arc<AtomicUsize>,
}

/// 用指定 Stub 构造 SubmissionService，返回服务、计数器与事件总线。
///
/// 服务被 `Guarded` 包裹：方法调用经 `Deref` 原样转发，同时把临时存储根
/// 的生命周期绑到服务上（服务被 Drop 时目录一起回收）。
fn build_service(mode: StubMode) -> (Guarded<SubmissionService>, StubCounters, Arc<CoreEventBus>) {
    let counters = StubCounters {
        judgement: Arc::new(AtomicUsize::new(0)),
        detail: Arc::new(AtomicUsize::new(0)),
        cases: Arc::new(AtomicUsize::new(0)),
    };
    let provider = Arc::new(StubSubmissionProvider {
        mode,
        calls: Arc::clone(&counters.judgement),
        detail_calls: Arc::clone(&counters.detail),
        cases_calls: Arc::clone(&counters.cases),
    });
    let registry: Arc<dyn ProviderRegistry> = Arc::new(ProviderRegistryImpl::new(OjId::new("HOJ")));
    registry.register(
        OjId::new("HOJ"),
        ProviderSet {
            submission: Some(provider),
            ..Default::default()
        },
    );
    let bus = Arc::new(CoreEventBus::new());
    let (storage, dir) = temp_storage();
    let service = SubmissionService::new(registry, Arc::clone(&bus), storage);
    (Guarded::new(service, dir), counters, bus)
}

fn make_service(
    mode: StubMode,
) -> (
    Guarded<SubmissionService>,
    Arc<AtomicUsize>,
    Arc<CoreEventBus>,
) {
    let (service, counters, bus) = build_service(mode);
    (service, counters.judgement, bus)
}

/// 订阅总线（事件断言用；`try_recv` 同步取，负向断言因此也是确定的）。
fn subscribe(bus: &Arc<CoreEventBus>) -> tokio::sync::broadcast::Receiver<CoreEvent> {
    bus.subscribe()
}

/// 取出当前已缓冲的提交相关事件（非阻塞）。
fn drain_submission_events(rx: &mut tokio::sync::broadcast::Receiver<CoreEvent>) -> Vec<CoreEvent> {
    let mut out = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if matches!(
            event,
            CoreEvent::SubmissionCreated { .. } | CoreEvent::SubmissionJudged { .. }
        ) {
            out.push(event);
        }
    }
    out
}

// ── 错误变体必须穿透 Service 层 ──
//
// 变体是前端 sessionGuard 判定会话失效的唯一依据（见 core/error.rs 的 context() 文档）。
// 提交是赛场上最不能失败的操作：token 过期时若被改写成 Submission 变体，
// 选手只会看到一条「提交失败」文案，反复重试全部失败，却永远不会被带回登录页。

#[test]
fn submit_preserves_auth_variant() {
    let (service, _calls, _bus) = make_service(StubMode::Auth);
    let err = block_on(service.submit("1011", "1061", "A", "C++", "int main(){}"))
        .expect_err("token 过期应报错");
    assert!(
        matches!(err, AppError::Auth(_)),
        "submit 必须保留 Auth 变体，实际 {:?}",
        err
    );
    assert!(
        err.user_message().contains("提交失败"),
        "应补上环节名: {}",
        err.user_message()
    );
}

#[test]
fn submit_preserves_network_variant() {
    let (service, _calls, _bus) = make_service(StubMode::Network);
    let err = block_on(service.submit("1011", "1061", "A", "C++", "int main(){}"))
        .expect_err("断网应报错");
    assert!(
        matches!(err, AppError::Network(_)),
        "断网不应被改写成 Submission 变体（否则「连不上」会显示成「提交被拒」），实际 {:?}",
        err
    );
}

// ── get_judgement 单次查询契约 ──
//
// 后端不循环、不睡眠、不设 deadline：轮询节拍 / 总超时 / 瞬时错误容忍
// 全部由前端 submissionStore 的 createPoller 编排。这里锁定单次查询的
// 三个契约：结果原样透传、错误变体穿透、终态才发 Judged 事件。

#[test]
fn get_judgement_returns_result_passthrough() {
    let (service, calls, _bus) = make_service(StubMode::Ok);
    let result = block_on(service.get_judgement("submit-1")).expect("应成功");
    assert!(
        matches!(result.status, JudgementStatus::Accepted),
        "实际 {:?}",
        result.status
    );
    assert_eq!(result.score, 100.0);
    assert_eq!(result.time_ms, 15);
    assert_eq!(result.memory_kb, 2048);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "单次查询只调 Provider 一次"
    );
}

#[test]
fn get_judgement_preserves_auth_variant() {
    let (service, calls, _bus) = make_service(StubMode::Auth);
    let err = block_on(service.get_judgement("submit-1")).expect_err("token 过期应报错");
    assert!(
        matches!(err, AppError::Auth(_)),
        "Auth 变体是前端 sessionGuard 判定会话失效的依据，不得改写，实际 {:?}",
        err
    );
    assert!(
        err.user_message().contains("评测查询失败"),
        "应补上环节名: {}",
        err.user_message()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1, "后端不重试，只查一次");
}

#[test]
fn get_judgement_preserves_network_variant() {
    let (service, _calls, _bus) = make_service(StubMode::Network);
    let err = block_on(service.get_judgement("submit-1")).expect_err("断网应报错");
    assert!(
        matches!(err, AppError::Network(_)),
        "瞬时错误的容忍与重试由前端 poller 负责，变体不得改写，实际 {:?}",
        err
    );
}

#[test]
fn get_judgement_publishes_judged_on_terminal_status() {
    let (service, _calls, bus) = make_service(StubMode::Ok);
    let mut rx = subscribe(&bus);
    let result = block_on(service.get_judgement("submit-1")).expect("应成功");
    let events = drain_submission_events(&mut rx);
    assert_eq!(events.len(), 1, "终态应发布一条事件，实际 {events:?}");
    assert_eq!(
        events[0],
        CoreEvent::SubmissionJudged {
            submission_id: "submit-1".into(),
            status: result.status.as_str().to_string(),
        },
        "载荷只带状态摘要（不带耗时/内存/测试点明细）"
    );
}

#[test]
fn get_judgement_publishes_nothing_on_non_terminal_status() {
    // 非终态（Running）原样透传、不发事件：是否继续轮询由前端决定
    let (service, _calls, bus) = make_service(StubMode::AlwaysRunning);
    let mut rx = subscribe(&bus);
    let result = block_on(service.get_judgement("submit-1")).expect("非终态也应成功返回");
    assert!(
        matches!(result.status, JudgementStatus::Running),
        "实际 {:?}",
        result.status
    );
    assert!(
        drain_submission_events(&mut rx).is_empty(),
        "非终态不应发布任何事件"
    );
}

// ── 提交列表 / 详情 / 测试点：错误变体穿透 + 正常路径 ──

fn sample_query() -> SubmissionQuery {
    SubmissionQuery {
        contest_id: "1011".into(),
        current_page: 1,
        limit: 20,
        only_mine: true,
        problem_display_id: None,
        status: None,
    }
}

#[test]
fn list_contest_submissions_preserves_auth_variant() {
    let (service, _calls, _bus) = make_service(StubMode::Auth);
    let err =
        block_on(service.list_contest_submissions(&sample_query())).expect_err("token 过期应报错");
    assert!(
        matches!(err, AppError::Auth(_)),
        "提交列表是认证调用，401 必须触发会话守卫，实际 {:?}",
        err
    );
    assert!(
        err.user_message().contains("获取提交列表失败"),
        "应补上环节名: {}",
        err.user_message()
    );
}

#[test]
fn list_contest_submissions_returns_page_on_success() {
    let (service, _calls, _bus) = make_service(StubMode::Ok);
    let page = block_on(service.list_contest_submissions(&sample_query())).expect("应成功");
    assert_eq!(page.records.len(), 1);
    assert_eq!(page.records[0].submit_id, "12345");
}

#[test]
fn get_submission_detail_preserves_auth_variant() {
    let (service, _calls, _bus) = make_service(StubMode::Auth);
    let err = block_on(service.get_submission_detail("12345")).expect_err("token 过期应报错");
    assert!(matches!(err, AppError::Auth(_)), "实际 {:?}", err);
    assert!(
        err.user_message().contains("获取提交详情失败"),
        "应补上环节名: {}",
        err.user_message()
    );
}

#[test]
fn get_submission_detail_returns_entity_on_success() {
    let (service, _calls, _bus) = make_service(StubMode::Ok);
    let detail = block_on(service.get_submission_detail("12345")).expect("应成功");
    assert_eq!(detail.submit_id, "12345");
    assert_eq!(detail.code, "int main(){}");
    assert_eq!(detail.error_message.as_deref(), Some("expected ';'"));
}

#[test]
fn get_submission_cases_preserves_auth_variant() {
    let (service, _calls, _bus) = make_service(StubMode::Auth);
    let err = block_on(service.get_submission_cases("12345")).expect_err("token 过期应报错");
    assert!(matches!(err, AppError::Auth(_)), "实际 {:?}", err);
    assert!(
        err.user_message().contains("获取测试点结果失败"),
        "应补上环节名: {}",
        err.user_message()
    );
}

#[test]
fn get_submission_cases_preserves_network_variant() {
    let (service, _calls, _bus) = make_service(StubMode::Network);
    let err = block_on(service.get_submission_cases("12345")).expect_err("断网应报错");
    assert!(
        matches!(err, AppError::Network(_)),
        "断网不应被改写成 Submission 变体，实际 {:?}",
        err
    );
}

#[test]
fn get_submission_cases_returns_cases_on_success() {
    let (service, _calls, _bus) = make_service(StubMode::Ok);
    let cases = block_on(service.get_submission_cases("12345")).expect("应成功");
    assert_eq!(cases.cases.len(), 1);
    assert_eq!(cases.mode, "default");
}

// ── 终态提交详情/测试点缓存（仅内存 + 登出清理）──
//
// 终态结果不可变，缓存可省掉「列表 → 详情 → 返回 → 再进」的重复请求；
// 评测中的结果**永不缓存**（否则界面停在「评测中」/ 明细停在半截）；
// 用户域数据不落盘，登出必须清空。

#[test]
fn detail_cache_key_carries_oj_scope_so_cross_oj_never_hits() {
    // 提交缓存键带 OJ 维度：submit_id 是各 OJ 自增的资源号，必然重号；
    // 键隔离让跨 OJ 串号在结构上不可能（不依赖「切 OJ 时清缓存」）
    let counters = StubCounters {
        judgement: Arc::new(AtomicUsize::new(0)),
        detail: Arc::new(AtomicUsize::new(0)),
        cases: Arc::new(AtomicUsize::new(0)),
    };
    let provider = Arc::new(StubSubmissionProvider {
        mode: StubMode::Ok,
        calls: Arc::clone(&counters.judgement),
        detail_calls: Arc::clone(&counters.detail),
        cases_calls: Arc::clone(&counters.cases),
    });
    let registry: Arc<dyn ProviderRegistry> = Arc::new(ProviderRegistryImpl::new(OjId::new("HOJ")));
    for id in ["HOJ", "QDUOJ"] {
        registry.register(
            OjId::new(id),
            ProviderSet {
                submission: Some(Arc::clone(&provider) as Arc<dyn SubmissionProvider>),
                ..Default::default()
            },
        );
    }
    let (storage, _dir) = temp_storage();
    let service = SubmissionService::new(
        Arc::clone(&registry),
        Arc::new(CoreEventBus::new()),
        storage,
    );

    block_on(service.get_submission_detail("12345")).expect("HOJ 详情失败");
    assert_eq!(counters.detail.load(Ordering::SeqCst), 1);
    block_on(service.get_submission_detail("12345")).expect("HOJ 详情二次失败");
    assert_eq!(
        counters.detail.load(Ordering::SeqCst),
        1,
        "同一 OJ 应命中缓存"
    );

    // 切 OJ：同一 submit_id 必须重新请求（不得命中上一个 OJ 的详情缓存）
    registry.set_current(OjId::new("QDUOJ"));
    block_on(service.get_submission_detail("12345")).expect("换 OJ 后详情失败");
    assert_eq!(
        counters.detail.load(Ordering::SeqCst),
        2,
        "跨 OJ 不得命中同一键（会读到另一 OJ 的提交内容）"
    );
}

#[test]
fn terminal_detail_is_cached() {
    let (service, counters, _bus) = build_service(StubMode::Ok);

    block_on(service.get_submission_detail("12345")).expect("首次应成功");
    block_on(service.get_submission_detail("12345")).expect("二次应成功");

    assert_eq!(
        counters.detail.load(Ordering::SeqCst),
        1,
        "终态详情应命中缓存"
    );
}

#[test]
fn non_terminal_detail_is_never_cached() {
    let (service, counters, _bus) = build_service(StubMode::NonTerminalDetail);

    block_on(service.get_submission_detail("12345")).expect("首次应成功");
    block_on(service.get_submission_detail("12345")).expect("二次应成功");

    assert_eq!(
        counters.detail.load(Ordering::SeqCst),
        2,
        "评测中的详情会变（状态/耗时/内存），不得缓存"
    );
}

#[test]
fn detail_errors_are_not_cached() {
    let (service, counters, _bus) = build_service(StubMode::Auth);

    let first = block_on(service.get_submission_detail("12345")).expect_err("应报错");
    let second = block_on(service.get_submission_detail("12345")).expect_err("应再次报错");

    assert!(matches!(first, AppError::Auth(_)), "实际 {:?}", first);
    assert!(matches!(second, AppError::Auth(_)), "实际 {:?}", second);
    assert_eq!(
        counters.detail.load(Ordering::SeqCst),
        2,
        "错误不得入缓存（否则会话恢复后仍返回旧错误）"
    );
}

#[test]
fn cases_cached_after_terminal_detail() {
    // 详情页流程：先拉详情（终态 → 留终态标记）再拉测试点 → 测试点可缓存
    let (service, counters, _bus) = build_service(StubMode::Ok);

    block_on(service.get_submission_detail("12345")).expect("详情应成功");
    block_on(service.get_submission_cases("12345")).expect("测试点应成功");
    block_on(service.get_submission_cases("12345")).expect("测试点二次应成功");

    assert_eq!(
        counters.cases.load(Ordering::SeqCst),
        1,
        "终态测试点应命中缓存"
    );
}

#[test]
fn cases_not_cached_without_terminal_mark() {
    // 控制台条的失败测试点提示直接拉测试点（无详情上下文）：
    // 不缓存 —— 请求数与改造前一致，而不是为判定终态多发一次详情请求
    let (service, counters, _bus) = build_service(StubMode::Ok);

    block_on(service.get_submission_cases("12345")).expect("测试点应成功");
    block_on(service.get_submission_cases("12345")).expect("测试点二次应成功");

    assert_eq!(counters.cases.load(Ordering::SeqCst), 2);
}

#[test]
fn non_terminal_detail_does_not_enable_cases_cache() {
    let (service, counters, _bus) = build_service(StubMode::NonTerminalDetail);

    block_on(service.get_submission_detail("12345")).expect("详情应成功");
    block_on(service.get_submission_cases("12345")).expect("测试点应成功");
    block_on(service.get_submission_cases("12345")).expect("测试点二次应成功");

    assert_eq!(
        counters.cases.load(Ordering::SeqCst),
        2,
        "评测中的测试点是半截明细，缓存它会让「评测完成后打开详情」缺测试点"
    );
}

#[test]
fn detail_cache_isolates_submissions() {
    let (service, counters, _bus) = build_service(StubMode::Ok);

    block_on(service.get_submission_detail("1")).expect("应成功");
    block_on(service.get_submission_detail("2")).expect("应成功");

    assert_eq!(
        counters.detail.load(Ordering::SeqCst),
        2,
        "不同提交不得互相命中"
    );
}

#[test]
fn clear_user_caches_empties_terminal_caches() {
    // 登出编排（auth_cmd::logout）依赖本方法：缓存含源代码，换账号不得复用
    let (service, counters, _bus) = build_service(StubMode::Ok);

    block_on(service.get_submission_detail("12345")).expect("详情应成功");
    block_on(service.get_submission_cases("12345")).expect("测试点应成功");
    assert_eq!(counters.detail.load(Ordering::SeqCst), 1);
    assert_eq!(counters.cases.load(Ordering::SeqCst), 1);

    service.clear_user_caches();

    block_on(service.get_submission_detail("12345")).expect("登出后应重新拉取详情");
    block_on(service.get_submission_cases("12345")).expect("登出后应重新拉取测试点");
    assert_eq!(counters.detail.load(Ordering::SeqCst), 2);
    // 终态标记同时被清：测试点缓存需重新建立上下文，故此处仍是 2 次
    assert_eq!(counters.cases.load(Ordering::SeqCst), 2);
}

/// `on_oj_switched()` 显式清用户域缓存（与登出同一语义：换 OJ 即换用户上下文）。
///
/// 旧实现靠订阅 `OJSwitched` 事件清理，正确性依赖事件投递时序；现在由
/// `switch_oj` 命令显式调用，返回即已清完。
#[test]
fn on_oj_switched_clears_user_scoped_caches() {
    let (service, counters, _bus) = build_service(StubMode::Ok);

    block_on(service.get_submission_detail("12345")).expect("详情应成功");
    block_on(service.get_submission_cases("12345")).expect("测试点应成功");
    assert_eq!(counters.detail.load(Ordering::SeqCst), 1);
    assert_eq!(counters.cases.load(Ordering::SeqCst), 1);

    service.on_oj_switched();

    // 返回即已清完：后续查询必然重新请求（旧 OJ 用户的提交内容不得复用）
    block_on(service.get_submission_detail("12345")).expect("切 OJ 后应重新拉取详情");
    assert_eq!(counters.detail.load(Ordering::SeqCst), 2);
}

/// `submit` 发布 `SubmissionCreated`（事实通知），且载荷只带 submission_id。
#[test]
fn submit_publishes_submission_created() {
    let (service, _calls, bus) = build_service(StubMode::Ok);
    let mut rx = subscribe(&bus);

    let submission_id =
        block_on(service.submit("1011", "p-1", "A", "C++", "int main(){}")).expect("提交应成功");

    let events = drain_submission_events(&mut rx);
    assert_eq!(
        events,
        vec![CoreEvent::SubmissionCreated {
            submission_id: submission_id.clone(),
        }],
        "应发布且仅发布一条 Created 事件"
    );
    // 源代码绝不进入事件流
    assert!(!format!("{events:?}").contains("int main"));
}
