use super::*;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::core::entity::submission::{
    JudgeCase, JudgementResult, JudgementStatus, SubmissionCases, SubmissionDetail, SubmissionPage,
    SubmissionQuery, SubmissionRecord,
};
use crate::core::error::AppError;
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
    /// 查询永远返回 Running（用于触发轮询超时）
    AlwaysRunning,
    /// 查询前 `n` 次返回 Network 错误，之后成功（模拟瞬时抖动后恢复）
    FlakyThenOk(usize),
    /// 查询前 `n` 次返回 Pending（排队中），之后返回终态 Accepted
    PendingThenOk(usize),
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

fn pending() -> JudgementResult {
    JudgementResult {
        status: JudgementStatus::Pending,
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
            StubMode::Ok | StubMode::AlwaysRunning | StubMode::FlakyThenOk(_)
            | StubMode::PendingThenOk(_) => Ok("submit-1".into()),
            StubMode::Auth => Err(AppError::Auth("stub: HTTP 401 Unauthorized".into())),
            StubMode::Network => Err(AppError::Network("stub: connection reset".into())),
        }
    }

    async fn get_judgement(&self, _submission_id: &str) -> AppResult<JudgementResult> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        match self.mode {
            StubMode::Ok => Ok(accepted()),
            StubMode::Auth => Err(AppError::Auth("stub: HTTP 401 Unauthorized".into())),
            StubMode::Network => Err(AppError::Network("stub: connection reset".into())),
            StubMode::AlwaysRunning => Ok(running()),
            StubMode::PendingThenOk(pending_count) => {
                if n < pending_count {
                    Ok(pending())
                } else {
                    Ok(accepted())
                }
            }
            StubMode::FlakyThenOk(failures) => {
                if n < failures {
                    Err(AppError::Network("stub: 瞬时抖动".into()))
                } else {
                    Ok(accepted())
                }
            }
        }
    }

    async fn list_contest_submissions(
        &self,
        _query: &SubmissionQuery,
    ) -> AppResult<SubmissionPage> {
        match self.mode {
            StubMode::Ok | StubMode::AlwaysRunning | StubMode::FlakyThenOk(_)
            | StubMode::PendingThenOk(_) => {
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
            StubMode::Ok | StubMode::AlwaysRunning | StubMode::FlakyThenOk(_)
            | StubMode::PendingThenOk(_) => {
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
            StubMode::Ok | StubMode::AlwaysRunning | StubMode::FlakyThenOk(_)
            | StubMode::PendingThenOk(_) => {
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

fn make_service(mode: StubMode) -> (SubmissionService, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let provider = Arc::new(StubSubmissionProvider {
        mode,
        calls: Arc::clone(&calls),
    });
    let registry: Arc<dyn ProviderRegistry> = Arc::new(ProviderRegistryImpl::new(OJType::HOJ));
    registry.register_submission(OJType::HOJ, provider);
    (
        SubmissionService::new(registry, Arc::new(EventBus::new())),
        calls,
    )
}

// ── 错误变体必须穿透 Service 层 ──
//
// 变体是前端 sessionGuard 判定会话失效的唯一依据（见 core/error.rs 的 context() 文档）。
// 提交是赛场上最不能失败的操作：token 过期时若被改写成 Submission 变体，
// 选手只会看到一条「提交失败」文案，反复重试全部失败，却永远不会被带回登录页。

#[test]
fn submit_preserves_auth_variant() {
    let (service, _calls) = make_service(StubMode::Auth);
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
    let (service, _calls) = make_service(StubMode::Network);
    let err = block_on(service.submit("1011", "1061", "C++", "int main(){}"))
        .expect_err("断网应报错");
    assert!(
        matches!(err, AppError::Network(_)),
        "断网不应被改写成 Submission 变体（否则「连不上」会显示成「提交被拒」），实际 {:?}",
        err
    );
}

// ── poll_judgement 的错误处理 ──

#[test]
fn poll_judgement_propagates_auth_immediately_without_waiting_for_timeout() {
    let (service, calls) = make_service(StubMode::Auth);

    // 超时给到 300s、间隔 60s：若认证错误被当成瞬时错误重试，
    // 这里会挂住五分钟并最终报「评测超时」（Submission 变体），守卫拿不到 Auth
    let started = std::time::Instant::now();
    let err = block_on(service.poll_judgement("submit-1", 60, 300))
        .expect_err("认证失败应上抛");
    let elapsed = started.elapsed();

    assert!(
        matches!(err, AppError::Auth(_)),
        "认证失败必须保留 Auth 变体，实际 {:?}",
        err
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "认证错误不应重试（重试也只是反复 401）"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "应立即返回而不是等到超时，实际耗时 {:?}",
        elapsed
    );
}

#[test]
fn poll_judgement_retries_transient_network_error_then_succeeds() {
    // 瞬时抖动仍要重试：这是轮询存在的意义，不能因为上面的修复而一并丢掉
    let (service, calls) = make_service(StubMode::FlakyThenOk(2));
    let result = block_on(service.poll_judgement("submit-1", 0, 60))
        .expect("抖动恢复后应拿到结果");
    assert!(
        matches!(result.status, JudgementStatus::Accepted),
        "抖动恢复后应拿到终态结果，实际 {:?}",
        result.status
    );
    assert_eq!(result.time_ms, 15);
    assert_eq!(result.memory_kb, 2048);
    assert_eq!(calls.load(Ordering::SeqCst), 3, "前两次失败 + 第三次成功");
}

#[test]
fn poll_judgement_timeout_is_submission_variant() {
    let (service, _calls) = make_service(StubMode::AlwaysRunning);
    let err = block_on(service.poll_judgement("submit-1", 0, 0))
        .expect_err("一直 Running 应超时");
    assert!(
        matches!(err, AppError::Submission(_)),
        "超时是评测语义问题，应为 Submission 变体，实际 {:?}",
        err
    );
    assert!(err.user_message().contains("评测超时"));
}

#[test]
fn poll_judgement_returns_terminal_result() {
    let (service, calls) = make_service(StubMode::Ok);
    let result = block_on(service.poll_judgement("submit-1", 0, 60)).expect("应成功");
    assert!(
        matches!(result.status, JudgementStatus::Accepted),
        "实际 {:?}",
        result.status
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1, "终态应一次返回，不再轮询");
}

#[test]
fn poll_judgement_keeps_polling_on_pending() {
    // get_judgement 现原样透传非终态：排队中的 Pending 不是终态，
    // 轮询必须继续而不是把 Pending 当最终结果返回
    let (service, calls) = make_service(StubMode::PendingThenOk(2));
    let result = block_on(service.poll_judgement("submit-1", 0, 60))
        .expect("排队结束后应拿到终态");
    assert!(
        matches!(result.status, JudgementStatus::Accepted),
        "实际 {:?}",
        result.status
    );
    assert_eq!(calls.load(Ordering::SeqCst), 3, "两次 Pending + 第三次终态");
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
    let (service, _calls) = make_service(StubMode::Auth);
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
    let (service, _calls) = make_service(StubMode::Ok);
    let page = block_on(service.list_contest_submissions(&sample_query())).expect("应成功");
    assert_eq!(page.records.len(), 1);
    assert_eq!(page.records[0].submit_id, "12345");
}

#[test]
fn get_submission_detail_preserves_auth_variant() {
    let (service, _calls) = make_service(StubMode::Auth);
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
    let (service, _calls) = make_service(StubMode::Ok);
    let detail = block_on(service.get_submission_detail("12345")).expect("应成功");
    assert_eq!(detail.submit_id, "12345");
    assert_eq!(detail.code, "int main(){}");
    assert_eq!(detail.error_message.as_deref(), Some("expected ';'"));
}

#[test]
fn get_submission_cases_preserves_auth_variant() {
    let (service, _calls) = make_service(StubMode::Auth);
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
    let (service, _calls) = make_service(StubMode::Network);
    let err = block_on(service.get_submission_cases("12345")).expect_err("断网应报错");
    assert!(
        matches!(err, AppError::Network(_)),
        "断网不应被改写成 Submission 变体，实际 {:?}",
        err
    );
}

#[test]
fn get_submission_cases_returns_cases_on_success() {
    let (service, _calls) = make_service(StubMode::Ok);
    let cases = block_on(service.get_submission_cases("12345")).expect("应成功");
    assert_eq!(cases.cases.len(), 1);
    assert_eq!(cases.mode, "default");
}
