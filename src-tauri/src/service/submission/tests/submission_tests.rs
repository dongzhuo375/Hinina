use super::*;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::core::entity::submission::{
    JudgeCase, JudgementResult, JudgementStatus, SubmissionCases, SubmissionDetail, SubmissionPage,
    SubmissionQuery, SubmissionRecord,
};
use crate::core::error::AppError;
use crate::core::event::event_category::EventCategory;
use crate::core::provider::oj_type::OJType;
use crate::core::provider::registry::ProviderRegistry;
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
}

struct StubSubmissionProvider {
    mode: StubMode,
    /// get_judgement 调用次数
    calls: Arc<AtomicUsize>,
}

fn accepted() -> JudgementResult {
    JudgementResult {
        status: JudgementStatus::Accepted,
        score: 100.0,
        time_ms: 15,
        memory_kb: 2048,
    }
}

fn running() -> JudgementResult {
    JudgementResult {
        status: JudgementStatus::Running,
        score: 0.0,
        time_ms: 0,
        memory_kb: 0,
    }
}

#[async_trait::async_trait]
impl SubmissionProvider for StubSubmissionProvider {
    async fn submit(
        &self,
        _contest_id: &str,
        _problem_id: &str,
        _language: &str,
        _source_code: &str,
    ) -> AppResult<String> {
        match self.mode {
            StubMode::Ok | StubMode::AlwaysRunning => Ok("submit-1".into()),
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
            StubMode::AlwaysRunning => Ok(running()),
        }
    }

    async fn list_contest_submissions(
        &self,
        _query: &SubmissionQuery,
    ) -> AppResult<SubmissionPage> {
        match self.mode {
            StubMode::Ok | StubMode::AlwaysRunning => {
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
        match self.mode {
            StubMode::Ok | StubMode::AlwaysRunning => {
                Ok(SubmissionDetail {
                    submit_id: "12345".into(),
                    pid: "1061".into(),
                    display_pid: "HOJ-1061".into(),
                    username: "alice".into(),
                    submit_time: 1_700_000_000,
                    status: JudgementStatus::CompilationError,
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
        match self.mode {
            StubMode::Ok | StubMode::AlwaysRunning => {
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

fn make_service(mode: StubMode) -> (SubmissionService, Arc<AtomicUsize>, Arc<EventBus>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let provider = Arc::new(StubSubmissionProvider {
        mode,
        calls: Arc::clone(&calls),
    });
    let registry: Arc<dyn ProviderRegistry> = Arc::new(ProviderRegistryImpl::new(OJType::HOJ));
    registry.register_submission(OJType::HOJ, provider);
    let bus = Arc::new(EventBus::new());
    (
        SubmissionService::new(registry, Arc::clone(&bus)),
        calls,
        bus,
    )
}

/// 订阅 Submission 类事件并收集，供事件契约断言。
fn collect_submission_events(bus: &Arc<EventBus>) -> Arc<Mutex<Vec<SubmissionEvent>>> {
    let events: Arc<Mutex<Vec<SubmissionEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);
    bus.subscribe(
        EventCategory::Submission,
        Arc::new(move |event: &AppEvent| {
            if let AppEvent::Submission(sub) = event {
                sink.lock().unwrap().push(sub.clone());
            }
        }),
    );
    events
}

// ── 错误变体必须穿透 Service 层 ──
//
// 变体是前端 sessionGuard 判定会话失效的唯一依据（见 core/error.rs 的 context() 文档）。
// 提交是赛场上最不能失败的操作：token 过期时若被改写成 Submission 变体，
// 选手只会看到一条「提交失败」文案，反复重试全部失败，却永远不会被带回登录页。

#[test]
fn submit_preserves_auth_variant() {
    let (service, _calls, _bus) = make_service(StubMode::Auth);
    let err = block_on(service.submit("1011", "1061", "C++", "int main(){}"))
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
    let err = block_on(service.submit("1011", "1061", "C++", "int main(){}"))
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
    assert_eq!(calls.load(Ordering::SeqCst), 1, "单次查询只调 Provider 一次");
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
    let events = collect_submission_events(&bus);
    let _ = block_on(service.get_judgement("submit-1")).expect("应成功");
    let events = events.lock().unwrap();
    assert_eq!(events.len(), 1, "终态应发布一条事件，实际 {:?}", *events);
    assert!(
        matches!(&events[0], SubmissionEvent::Judged { submission_id, .. } if submission_id == "submit-1"),
        "应为 Judged 且携带 submission_id，实际 {:?}",
        events[0]
    );
}

#[test]
fn get_judgement_publishes_nothing_on_non_terminal_status() {
    // 非终态（Running）原样透传、不发事件：是否继续轮询由前端决定
    let (service, _calls, bus) = make_service(StubMode::AlwaysRunning);
    let events = collect_submission_events(&bus);
    let result = block_on(service.get_judgement("submit-1")).expect("非终态也应成功返回");
    assert!(
        matches!(result.status, JudgementStatus::Running),
        "实际 {:?}",
        result.status
    );
    assert!(
        events.lock().unwrap().is_empty(),
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
    let err = block_on(service.list_contest_submissions(&sample_query()))
        .expect_err("token 过期应报错");
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
    assert!(
        matches!(err, AppError::Auth(_)),
        "实际 {:?}",
        err
    );
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
    assert!(
        matches!(err, AppError::Auth(_)),
        "实际 {:?}",
        err
    );
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
