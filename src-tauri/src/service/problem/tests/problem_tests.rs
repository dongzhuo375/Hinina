use super::*;

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::core::entity::problem::{Problem, Sample};
use crate::core::provider::oj_type::OJType;
use crate::core::provider::problem::ProblemProvider;
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

/// Stub ProblemProvider：按 displayId 返回固定 limits，并记录调用次数。
struct StubProblemProvider {
    /// get_problem 调用次数（用于断言缓存是否真的省掉了请求）
    calls: Arc<AtomicUsize>,
    /// 需要失败的 displayId（模拟 403「该比赛题目当前不可访问」）
    failing: Vec<String>,
    /// 为 true 时 `list_problems` / `get_user_problem_status` 也返回 Auth 错误，
    /// 用于断言 Service 层不改写错误变体（见「错误变体穿透」小节）
    fail_all: bool,
}

#[async_trait::async_trait]
impl ProblemProvider for StubProblemProvider {
    async fn get_problem(&self, _contest_id: &str, problem_id: &str) -> AppResult<Problem> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.failing.iter().any(|id| id == problem_id) {
            return Err(AppError::Auth("该比赛题目当前不可访问".into()));
        }
        Ok(Problem {
            id: format!("pid-{}", problem_id),
            title: format!("Problem {}", problem_id),
            description: String::new(),
            input_description: String::new(),
            output_description: String::new(),
            samples: Vec::<Sample>::new(),
            time_limit: 1000,
            memory_limit: 256,
            languages: Vec::new(),
        })
    }

    async fn list_problems(&self, _contest_id: &str) -> AppResult<Vec<Problem>> {
        if self.fail_all {
            return Err(AppError::Auth("stub: HTTP 401 Unauthorized".into()));
        }
        Ok(Vec::new())
    }

    async fn get_user_problem_status(
        &self,
        _contest_id: &str,
        problem_ids: &[String],
    ) -> AppResult<HashMap<String, i32>> {
        if self.fail_all {
            return Err(AppError::Auth("stub: HTTP 401 Unauthorized".into()));
        }
        Ok(problem_ids.iter().map(|id| (id.clone(), 0)).collect())
    }
}

/// 用指定 Stub 构造基于临时目录的 ProblemService。
fn build_service(
    dir: &std::path::Path,
    calls: Arc<AtomicUsize>,
    failing: Vec<String>,
) -> ProblemService {
    build_service_with(dir, calls, failing, false)
}

/// 同 `build_service`，但可让全部方法以 Auth 错误失败。
fn build_service_with(
    dir: &std::path::Path,
    calls: Arc<AtomicUsize>,
    failing: Vec<String>,
    fail_all: bool,
) -> ProblemService {
    let provider = Arc::new(StubProblemProvider {
        calls,
        failing,
        fail_all,
    });
    let registry: Arc<dyn ProviderRegistry> = Arc::new(ProviderRegistryImpl::new(OJType::HOJ));
    registry.register_problem(OJType::HOJ, provider);
    ProblemService::new(
        registry,
        Arc::new(EventBus::new()),
        Arc::new(Storage::new(dir.to_path_buf())),
    )
}

/// 构造基于独立临时目录的 ProblemService；返回服务、调用计数与目录。
fn make_service(
    test_name: &str,
    failing: Vec<String>,
) -> (ProblemService, Arc<AtomicUsize>, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("hinina-test-problem-{}", test_name));
    let _ = std::fs::remove_dir_all(&dir);
    let calls = Arc::new(AtomicUsize::new(0));
    let service = build_service(&dir, Arc::clone(&calls), failing);
    (service, calls, dir)
}

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_string()).collect()
}

fn call_count(calls: &AtomicUsize) -> usize {
    calls.load(Ordering::SeqCst)
}

// ── limits 缓存 ──

#[test]
fn limits_first_call_fetches_all_and_persists_to_disk() {
    let (service, calls, dir) = make_service("limits-first", Vec::new());

    let result = block_on(service.load_problem_limits("1", &ids(&["A", "B", "C"])))
        .expect("首次获取 limits 失败");

    assert_eq!(call_count(&calls), 3, "三道题应各请求一次详情");
    // 返回顺序必须与入参一致（前端按 displayId 对齐卡片）
    assert_eq!(
        result.iter().map(|l| l.display_id.as_str()).collect::<Vec<_>>(),
        vec!["A", "B", "C"]
    );
    assert_eq!(result[0].time_limit, 1000);
    assert_eq!(result[0].memory_limit, 256);
    assert!(
        dir.join("cache").join("problem_limits").join("1.json").exists(),
        "limits 应落盘以便重启后复用"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn limits_second_call_hits_memory_cache() {
    let (service, calls, dir) = make_service("limits-memory", Vec::new());
    let query = ids(&["A", "B"]);

    block_on(service.load_problem_limits("1", &query)).expect("首次失败");
    assert_eq!(call_count(&calls), 2);

    block_on(service.load_problem_limits("1", &query)).expect("二次失败");
    assert_eq!(call_count(&calls), 2, "内存缓存命中时不应再发请求");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn limits_disk_cache_survives_new_service_instance() {
    let dir = std::env::temp_dir().join("hinina-test-problem-limits-disk");
    let _ = std::fs::remove_dir_all(&dir);

    // 第一个实例：拉取并落盘
    let (first, first_calls, _) = make_service("limits-disk", Vec::new());
    block_on(first.load_problem_limits("1", &ids(&["A", "B"]))).expect("首次失败");
    assert_eq!(call_count(&first_calls), 2);

    // 第二个实例复用同一目录：应完全命中磁盘缓存（模拟客户端重启）
    let calls = Arc::new(AtomicUsize::new(0));
    let second = build_service(&dir, Arc::clone(&calls), Vec::new());

    let result = block_on(second.load_problem_limits("1", &ids(&["A", "B"]))).expect("重启后失败");
    assert_eq!(call_count(&calls), 0, "重启后应命中磁盘缓存，零请求");
    assert_eq!(result.len(), 2);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn limits_partial_failure_returns_successful_subset() {
    let (service, calls, dir) = make_service("limits-partial", vec!["B".to_string()]);

    let result = block_on(service.load_problem_limits("1", &ids(&["A", "B", "C"])))
        .expect("部分失败不应整体报错");

    assert_eq!(call_count(&calls), 3);
    // 失败的题目直接缺失，而不是回退成假默认值
    assert_eq!(
        result.iter().map(|l| l.display_id.as_str()).collect::<Vec<_>>(),
        vec!["A", "C"]
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn limits_all_failed_propagates_error_instead_of_defaults() {
    let (service, _calls, dir) =
        make_service("limits-all-fail", vec!["A".to_string(), "B".to_string()]);

    let error = block_on(service.load_problem_limits("1", &ids(&["A", "B"])))
        .expect_err("全部失败必须上抛，不能静默回退默认值");

    // 401/403 这类会话或权限问题必须让前端明确提示
    assert!(
        matches!(error, AppError::Auth(_)),
        "应保留原始错误类型，实际为 {:?}",
        error
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn limits_corrupted_cache_file_is_refetched() {
    let dir = std::env::temp_dir().join("hinina-test-problem-limits-corrupt");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("cache").join("problem_limits")).expect("创建缓存目录失败");
    std::fs::write(
        dir.join("cache").join("problem_limits").join("1.json"),
        "{ not valid json",
    )
    .expect("写入损坏缓存失败");

    let calls = Arc::new(AtomicUsize::new(0));
    let service = build_service(&dir, Arc::clone(&calls), Vec::new());

    let result =
        block_on(service.load_problem_limits("1", &ids(&["A"]))).expect("损坏缓存应降级重取");
    assert_eq!(call_count(&calls), 1, "损坏缓存必须重新获取");
    assert_eq!(result.len(), 1);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn limits_empty_input_returns_empty_without_request() {
    let (service, calls, dir) = make_service("limits-empty", Vec::new());

    let result = block_on(service.load_problem_limits("1", &[])).expect("空入参失败");
    assert!(result.is_empty());
    assert_eq!(call_count(&calls), 0);

    let _ = std::fs::remove_dir_all(&dir);
}

// ── 我的题目状态 ──

#[test]
fn user_problem_status_empty_input_skips_request() {
    let (service, _calls, dir) = make_service("status-empty", Vec::new());

    let result = block_on(service.get_user_problem_status("1", &[])).expect("空入参失败");
    assert!(result.is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn user_problem_status_maps_by_problem_id() {
    let (service, _calls, dir) = make_service("status-map", Vec::new());

    let result = block_on(service.get_user_problem_status("1", &ids(&["1001", "1002"])))
        .expect("获取状态失败");
    assert_eq!(result.get("1001"), Some(&0));
    assert_eq!(result.get("1002"), Some(&0));

    let _ = std::fs::remove_dir_all(&dir);
}

// ── 错误变体穿透 ──
//
// 变体是前端 sessionGuard 判定会话失效的唯一依据（见 core/error.rs 的 context() 文档）。
// Service 层若用 AppError::Problem(format!(...)) 重新包装，token 过期时选手只会看到
// 「题目错误」文案而不会被带回登录页，反复重试也全部失败。

/// 构造一个所有方法都以 Auth 失败的服务。
fn make_failing_service(test_name: &str) -> (ProblemService, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("hinina-test-problem-{}", test_name));
    let _ = std::fs::remove_dir_all(&dir);
    let service = build_service_with(&dir, Arc::new(AtomicUsize::new(0)), Vec::new(), true);
    (service, dir)
}

#[test]
fn open_problem_preserves_auth_variant() {
    // Stub 对 failing 列表内的 displayId 返回 Auth（模拟 401 / 私有赛未注册）
    let (service, _calls, _dir) = make_service("variant-open", ids(&["A"]));
    let err = block_on(service.open_problem("1011", "A")).expect_err("应报错");
    assert!(
        matches!(err, AppError::Auth(_)),
        "open_problem 必须保留 Auth 变体，实际 {:?}",
        err
    );
    assert!(
        err.user_message().contains("获取题目详情失败"),
        "应补上环节名: {}",
        err.user_message()
    );
}

#[test]
fn list_problems_preserves_auth_variant() {
    let (service, _dir) = make_failing_service("variant-list");
    let err = block_on(service.list_problems("1011")).expect_err("应报错");
    assert!(
        matches!(err, AppError::Auth(_)),
        "实际 {:?}",
        err
    );
}

#[test]
fn get_user_problem_status_preserves_auth_variant() {
    let (service, _dir) = make_failing_service("variant-status");
    // 空入参会短路返回空 map，必须传非空才能真正走到 Provider
    let err = block_on(service.get_user_problem_status("1011", &ids(&["1061", "1062"])))
        .expect_err("应报错");
    assert!(
        matches!(err, AppError::Auth(_)),
        "题目总览的「我的状态」每 30s 轮询一次，变体被改写会让守卫失灵，实际 {:?}",
        err
    );
}
