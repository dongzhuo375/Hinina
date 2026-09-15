use super::*;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::core::entity::submission::{JudgementResult, JudgementStatus};
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
            StubMode::Ok | StubMode::AlwaysRunning | StubMode::FlakyThenOk(_) => {
                Ok("submit-1".into())
            }
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
            StubMode::FlakyThenOk(failures) => {
                if n < failures {
                    Err(AppError::Network("stub: 瞬时抖动".into()))
                } else {
                    Ok(accepted())
                }
            }
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
