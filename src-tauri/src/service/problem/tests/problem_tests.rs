use super::*;

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::core::entity::problem::{Problem, Sample};
use crate::core::event::core_event::CoreEvent;
use crate::core::event::core_event_bus::CoreEventBus;
use crate::core::provider::oj_id::OjId;
use crate::core::provider::problem::ProblemProvider;
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

/// Stub ProblemProvider：按 displayId 返回固定 limits，并记录调用次数。
struct StubProblemProvider {
    /// get_problem 调用次数（用于断言缓存是否真的省掉了请求）
    calls: Arc<AtomicUsize>,
    /// 需要失败的 displayId（模拟 403「该比赛题目当前不可访问」）
    failing: Vec<String>,
    /// 为 true 时 `get_user_problem_status` 返回 Auth 错误，
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
    let registry: Arc<dyn ProviderRegistry> = Arc::new(ProviderRegistryImpl::new(OjId::new("HOJ")));
    registry.register(
        OjId::new("HOJ"),
        ProviderSet {
            problem: Some(provider),
            ..Default::default()
        },
    );
    ProblemService::new(
        registry,
        Arc::new(crate::core::event::core_event_bus::CoreEventBus::new()),
        Arc::new(Storage::new(dir.to_path_buf())),
    )
}

/// 构造基于独立临时目录的 ProblemService；返回服务、调用计数与目录。
fn make_service(
    test_name: &str,
    failing: Vec<String>,
) -> (ProblemService, Arc<AtomicUsize>, TempDir) {
    let dir = TempDir::named(&format!("hinina-test-problem-{}", test_name));
    let calls = Arc::new(AtomicUsize::new(0));
    let service = build_service(dir.path(), Arc::clone(&calls), failing);
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
        result
            .iter()
            .map(|l| l.display_id.as_str())
            .collect::<Vec<_>>(),
        vec!["A", "B", "C"]
    );
    assert_eq!(result[0].time_limit, 1000);
    assert_eq!(result[0].memory_limit, 256);
    assert!(
        dir.join("cache")
            .join("problem_limits")
            .join("HOJ")
            .join("1.json")
            .exists(),
        "limits 应落盘以便重启后复用（路径含 OJ 维度，跨 OJ 不撞号）"
    );
}

#[test]
fn limits_second_call_hits_memory_cache() {
    let (service, calls, _dir) = make_service("limits-memory", Vec::new());
    let query = ids(&["A", "B"]);

    block_on(service.load_problem_limits("1", &query)).expect("首次失败");
    assert_eq!(call_count(&calls), 2);

    block_on(service.load_problem_limits("1", &query)).expect("二次失败");
    assert_eq!(call_count(&calls), 2, "内存缓存命中时不应再发请求");
}

#[test]
fn limits_disk_cache_survives_new_service_instance() {
    let dir = TempDir::named("hinina-test-problem-limits-disk");

    // 第一个实例：拉取并落盘
    let (first, first_calls, _dir) = make_service("limits-disk", Vec::new());
    block_on(first.load_problem_limits("1", &ids(&["A", "B"]))).expect("首次失败");
    assert_eq!(call_count(&first_calls), 2);

    // 第二个实例复用同一目录：应完全命中磁盘缓存（模拟客户端重启）
    let calls = Arc::new(AtomicUsize::new(0));
    let second = build_service(dir.path(), Arc::clone(&calls), Vec::new());

    let result = block_on(second.load_problem_limits("1", &ids(&["A", "B"]))).expect("重启后失败");
    assert_eq!(call_count(&calls), 0, "重启后应命中磁盘缓存，零请求");
    assert_eq!(result.len(), 2);
}

#[test]
fn limits_partial_failure_returns_successful_subset() {
    let (service, calls, _dir) = make_service("limits-partial", vec!["B".to_string()]);

    let result = block_on(service.load_problem_limits("1", &ids(&["A", "B", "C"])))
        .expect("部分失败不应整体报错");

    assert_eq!(call_count(&calls), 3);
    // 失败的题目直接缺失，而不是回退成假默认值
    assert_eq!(
        result
            .iter()
            .map(|l| l.display_id.as_str())
            .collect::<Vec<_>>(),
        vec!["A", "C"]
    );
}

#[test]
fn limits_all_failed_propagates_error_instead_of_defaults() {
    let (service, _calls, _dir) =
        make_service("limits-all-fail", vec!["A".to_string(), "B".to_string()]);

    let error = block_on(service.load_problem_limits("1", &ids(&["A", "B"])))
        .expect_err("全部失败必须上抛，不能静默回退默认值");

    // 401/403 这类会话或权限问题必须让前端明确提示
    assert!(
        matches!(error, AppError::Auth(_)),
        "应保留原始错误类型，实际为 {:?}",
        error
    );
}

#[test]
fn limits_corrupted_cache_file_is_refetched() {
    let dir = TempDir::named("hinina-test-problem-limits-corrupt");

    let calls = Arc::new(AtomicUsize::new(0));
    // 先构造 service（其构造会做一次性布局清扫并落地标记），**再**写入损坏条目 ——
    // 反过来的话损坏文件会被布局清扫删掉，用例就退化成「缓存缺失」而非「缓存损坏」
    let service = build_service(dir.path(), Arc::clone(&calls), Vec::new());

    std::fs::create_dir_all(dir.join("cache").join("problem_limits").join("HOJ"))
        .expect("创建缓存目录失败");
    let corrupted = dir
        .join("cache")
        .join("problem_limits")
        .join("HOJ")
        .join("1.json");
    std::fs::write(&corrupted, "{ not valid json").expect("写入损坏缓存失败");
    assert!(corrupted.exists(), "预置失败：损坏条目未落盘");

    let result =
        block_on(service.load_problem_limits("1", &ids(&["A"]))).expect("损坏缓存应降级重取");
    assert_eq!(call_count(&calls), 1, "损坏缓存必须重新获取");
    assert_eq!(result.len(), 1);
    // 重取后条目被有效内容覆盖（损坏文件不该长期滞留）
    let rewritten = std::fs::read_to_string(&corrupted).expect("重取后应回写缓存");
    assert!(
        rewritten.contains("\"A\""),
        "重取后应以有效内容覆盖损坏条目，实际: {}",
        rewritten
    );
}

#[test]
fn limits_empty_input_returns_empty_without_request() {
    let (service, calls, _dir) = make_service("limits-empty", Vec::new());

    let result = block_on(service.load_problem_limits("1", &[])).expect("空入参失败");
    assert!(result.is_empty());
    assert_eq!(call_count(&calls), 0);
}

// ── 我的题目状态 ──

#[test]
fn user_problem_status_empty_input_skips_request() {
    let (service, _calls, _dir) = make_service("status-empty", Vec::new());

    let result = block_on(service.get_user_problem_status("1", &[])).expect("空入参失败");
    assert!(result.is_empty());
}

#[test]
fn user_problem_status_maps_by_problem_id() {
    let (service, _calls, _dir) = make_service("status-map", Vec::new());

    let result = block_on(service.get_user_problem_status("1", &ids(&["1001", "1002"])))
        .expect("获取状态失败");
    assert_eq!(result.get("1001"), Some(&0));
    assert_eq!(result.get("1002"), Some(&0));
}

// ── 题面缓存（内存 + 磁盘，受 oj.cacheProblemStatement 开关控制）──

#[test]
fn statement_cache_hit_skips_second_get_problem() {
    let (service, calls, _dir) = make_service("statement-hit", Vec::new());

    block_on(service.open_problem("1011", "A", true)).expect("首次打开失败");
    assert_eq!(call_count(&calls), 1);

    let problem = block_on(service.open_problem("1011", "A", true)).expect("二次打开失败");
    assert_eq!(call_count(&calls), 1, "题面应命中缓存，不再请求 Provider");
    assert_eq!(problem.id, "pid-A");
}

#[test]
fn statement_cache_disabled_always_fetches() {
    let (service, calls, dir) = make_service("statement-off", Vec::new());

    block_on(service.open_problem("1011", "A", false)).expect("首次打开失败");
    block_on(service.open_problem("1011", "A", false)).expect("二次打开失败");

    assert_eq!(call_count(&calls), 2, "关闭缓存后每次打开都直连服务端");
    assert!(
        !dir.join("cache").join("problem_statement").exists(),
        "关闭缓存时不得落盘（开关关闭 = 不读不写）"
    );
}

#[test]
fn statement_cache_survives_new_service_instance() {
    let dir = TempDir::named("hinina-test-problem-statement-disk");

    // 第一个实例：拉取并落盘
    let first = build_service(dir.path(), Arc::new(AtomicUsize::new(0)), Vec::new());
    block_on(first.open_problem("1011", "A", true)).expect("首次打开失败");
    drop(first);

    // 第二个实例复用同一目录：应命中磁盘缓存（模拟客户端重启）
    let calls = Arc::new(AtomicUsize::new(0));
    let second = build_service(dir.path(), Arc::clone(&calls), Vec::new());
    block_on(second.open_problem("1011", "A", true)).expect("重启后打开失败");

    assert_eq!(call_count(&calls), 0, "重启后应命中题面磁盘缓存，零请求");
}

#[test]
fn statement_cache_isolates_contest_and_problem() {
    let (service, calls, _dir) = make_service("statement-isolation", Vec::new());

    block_on(service.open_problem("1011", "A", true)).expect("打开失败");
    // 同一 displayId、不同比赛 = 不同题目
    block_on(service.open_problem("1012", "A", true)).expect("打开失败");
    // 同一比赛、不同题目
    block_on(service.open_problem("1011", "B", true)).expect("打开失败");

    assert_eq!(call_count(&calls), 3, "缓存键必须含比赛与题目两个维度");
}

#[test]
fn statement_cache_never_stores_errors() {
    let (service, calls, _dir) = make_service("statement-error", ids(&["A"]));

    let first = block_on(service.open_problem("1011", "A", true)).expect_err("应报错");
    let second = block_on(service.open_problem("1011", "A", true)).expect_err("应再次报错");

    assert!(matches!(first, AppError::Auth(_)), "实际 {:?}", first);
    assert!(matches!(second, AppError::Auth(_)), "实际 {:?}", second);
    assert_eq!(call_count(&calls), 2, "错误不得入缓存，重试必须重新请求");
}

// ── 错误变体穿透 ──
//
// 变体是前端 sessionGuard 判定会话失效的唯一依据（见 core/error.rs 的 context() 文档）。
// Service 层若用 AppError::Problem(format!(...)) 重新包装，token 过期时选手只会看到
// 「题目错误」文案而不会被带回登录页，反复重试也全部失败。

/// 构造一个所有方法都以 Auth 失败的服务。
fn make_failing_service(test_name: &str) -> (ProblemService, TempDir) {
    let dir = TempDir::named(&format!("hinina-test-problem-{}", test_name));
    let service = build_service_with(dir.path(), Arc::new(AtomicUsize::new(0)), Vec::new(), true);
    (service, dir)
}

#[test]
fn open_problem_preserves_auth_variant() {
    // Stub 对 failing 列表内的 displayId 返回 Auth（模拟 401 / 私有赛未注册）
    let (service, _calls, _dir) = make_service("variant-open", ids(&["A"]));
    let err = block_on(service.open_problem("1011", "A", true)).expect_err("应报错");
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

// ── OJSwitched：OJ 域缓存失效（键控不含 OJ 维度，切 OJ 防跨 OJ 撞号）──

#[test]
fn statement_cache_key_carries_oj_scope_so_cross_oj_never_hits() {
    // 这是「延迟清理磁盘缓存」之所以安全的前提：缓存键自带 OJ 维度，
    // 跨 OJ 同 cid/pid 在结构上不可能互相命中 —— 不依赖任何清理时机。
    let dir = TempDir::named("hinina-test-problem-oj-scope");
    let calls = Arc::new(AtomicUsize::new(0));
    let provider = Arc::new(StubProblemProvider {
        calls: Arc::clone(&calls),
        failing: Vec::new(),
        fail_all: false,
    });
    let registry: Arc<dyn ProviderRegistry> = Arc::new(ProviderRegistryImpl::new(OjId::new("HOJ")));
    // 同一个 Provider 同时注册到两个 OJ 下（数据相同也没关系 —— 我们要断言的是
    // 「请求有没有再发出去」，即缓存有没有跨 OJ 命中）
    for id in ["HOJ", "QDUOJ"] {
        registry.register(
            OjId::new(id),
            ProviderSet {
                problem: Some(Arc::clone(&provider) as Arc<dyn ProblemProvider>),
                ..Default::default()
            },
        );
    }
    let service = ProblemService::new(
        Arc::clone(&registry),
        Arc::new(crate::core::event::core_event_bus::CoreEventBus::new()),
        Arc::new(Storage::new(dir.to_path_buf())),
    );

    // HOJ：首拉 + 二次命中（内存）
    block_on(service.open_problem("1", "A", true)).expect("首次打开失败");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    block_on(service.open_problem("1", "A", true)).expect("二次打开失败");
    assert_eq!(calls.load(Ordering::SeqCst), 1, "同一 OJ 应命中缓存");

    // 切到另一个 OJ：同一 contest/display 必须重新请求（不命中 HOJ 的缓存）
    registry.set_current(OjId::new("QDUOJ"));
    block_on(service.open_problem("1", "A", true)).expect("换 OJ 后打开失败");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "跨 OJ 不得命中同一键（键缺 OJ 维度会让旧 OJ 数据驱动新 OJ 查询）"
    );

    // 新 OJ 自己的缓存正常工作
    block_on(service.open_problem("1", "A", true)).expect("新 OJ 二次打开失败");
    assert_eq!(calls.load(Ordering::SeqCst), 2);

    // 磁盘层同样隔离：换一个实例（模拟重启）读同一目录，仍是新 OJ 的键
    let calls_after_restart = Arc::new(AtomicUsize::new(0));
    let provider2 = Arc::new(StubProblemProvider {
        calls: Arc::clone(&calls_after_restart),
        failing: Vec::new(),
        fail_all: false,
    });
    let registry2: Arc<dyn ProviderRegistry> =
        Arc::new(ProviderRegistryImpl::new(OjId::new("QDUOJ")));
    registry2.register(
        OjId::new("QDUOJ"),
        ProviderSet {
            problem: Some(provider2),
            ..Default::default()
        },
    );
    let service2 = ProblemService::new(
        registry2,
        Arc::new(crate::core::event::core_event_bus::CoreEventBus::new()),
        Arc::new(Storage::new(dir.to_path_buf())),
    );
    block_on(service2.open_problem("1", "A", true)).expect("重启后打开失败");
    assert_eq!(
        calls_after_restart.load(Ordering::SeqCst),
        0,
        "重启后应命中本 OJ 的磁盘缓存（键含 OJ 维度）"
    );
}

#[test]
fn limits_disk_cache_key_carries_oj_scope() {
    // limits 的磁盘路径同样带 OJ 维度：`cache/problem_limits/{oj}/{cid}.json`
    let dir = TempDir::named("hinina-test-problem-limits-oj-scope");
    let calls = Arc::new(AtomicUsize::new(0));
    let provider = Arc::new(StubProblemProvider {
        calls: Arc::clone(&calls),
        failing: Vec::new(),
        fail_all: false,
    });
    let registry: Arc<dyn ProviderRegistry> = Arc::new(ProviderRegistryImpl::new(OjId::new("HOJ")));
    for id in ["HOJ", "QDUOJ"] {
        registry.register(
            OjId::new(id),
            ProviderSet {
                problem: Some(Arc::clone(&provider) as Arc<dyn ProblemProvider>),
                ..Default::default()
            },
        );
    }
    let service = ProblemService::new(
        Arc::clone(&registry),
        Arc::new(crate::core::event::core_event_bus::CoreEventBus::new()),
        Arc::new(Storage::new(dir.to_path_buf())),
    );

    block_on(service.load_problem_limits("1", &ids(&["A"]))).expect("HOJ limits 失败");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(
        dir.join("cache/problem_limits/HOJ/1.json").exists(),
        "HOJ 的 limits 落在自己的目录下"
    );

    registry.set_current(OjId::new("QDUOJ"));
    block_on(service.load_problem_limits("1", &ids(&["A"]))).expect("QDUOJ limits 失败");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "跨 OJ 不得命中 HOJ 的 limits 磁盘缓存"
    );
    assert!(
        dir.join("cache/problem_limits/QDUOJ/1.json").exists(),
        "QDUOJ 的 limits 落在自己的目录下"
    );
}

/// `on_oj_switched()` 显式清理题面 / limits 缓存（内存段同步，磁盘段同步兜底）。
///
/// 同步上下文（无 tokio runtime）走 `on_oj_switched` 的同步兜底分支 —— 返回即已清完。
/// 旧实现靠订阅 `OJSwitched` + 延迟队列清理，正确性依赖事件投递时序。
#[test]
fn on_oj_switched_clears_problem_scoped_caches() {
    let dir = TempDir::named("hinina-test-problem-oj-switch");
    let calls = Arc::new(AtomicUsize::new(0));
    let provider = Arc::new(StubProblemProvider {
        calls: Arc::clone(&calls),
        failing: Vec::new(),
        fail_all: false,
    });
    let registry: Arc<dyn ProviderRegistry> = Arc::new(ProviderRegistryImpl::new(OjId::new("HOJ")));
    registry.register(
        OjId::new("HOJ"),
        ProviderSet {
            problem: Some(Arc::clone(&provider) as Arc<dyn ProblemProvider>),
            ..Default::default()
        },
    );
    let service = ProblemService::new(
        registry,
        Arc::new(crate::core::event::core_event_bus::CoreEventBus::new()),
        Arc::new(Storage::new(dir.to_path_buf())),
    );

    // 预置内存 + **磁盘**缓存：磁盘条目必须真实落盘 —— 否则「目录不存在」的断言恒真
    block_on(service.open_problem("7", "A", true)).expect("预置题面失败");
    block_on(service.load_problem_limits("7", &ids(&["A"]))).expect("预置 limits 失败");
    let statement_entry = dir
        .join("cache")
        .join("problem_statement")
        .join("HOJ")
        .join("7")
        .join("A.json");
    let limits_entry = dir
        .join("cache")
        .join("problem_limits")
        .join("HOJ")
        .join("7.json");
    assert!(statement_entry.exists(), "预置失败：题面磁盘缓存未落盘");
    assert!(limits_entry.exists(), "预置失败：limits 磁盘缓存未落盘");
    assert!(!service.limits_cache.read().unwrap().is_empty());
    assert!(!service.statement_cache.is_empty());

    // 显式调用（由 `switch_oj` 命令触发）：不经过事件投递
    service.on_oj_switched();

    assert!(
        service.limits_cache.read().unwrap().is_empty(),
        "limits 内存缓存应被清空"
    );
    assert!(service.statement_cache.is_empty(), "题面内存缓存应被清空");
    assert!(
        !dir.join("cache/problem_statement").exists(),
        "题面磁盘缓存应被清理（同步上下文下立即完成）"
    );
    assert!(
        !dir.join("cache/problem_limits").exists(),
        "limits 磁盘缓存应被清理（同步上下文下立即完成）"
    );
}

// ── 设置页「清空缓存」（同步清理，不重拉） ──

#[test]
fn clear_caches_empties_statement_and_limits_caches() {
    let (service, calls, dir) = make_service("clear-caches", Vec::new());

    // 预置两层（内存 + 磁盘）：题面与 limits。磁盘条目必须真实落盘，
    // 否则「目录不存在」的断言恒真，等于零覆盖
    block_on(service.open_problem("1", "A", true)).expect("预置题面失败");
    block_on(service.load_problem_limits("1", &ids(&["A"]))).expect("预置 limits 失败");
    let calls_before = call_count(&calls);

    let statement_disk = dir.join("cache").join("problem_statement");
    let limits_disk = dir.join("cache").join("problem_limits");
    assert!(statement_disk.exists(), "预置失败：题面磁盘缓存未落盘");
    assert!(limits_disk.exists(), "预置失败：limits 磁盘缓存未落盘");
    assert!(
        !service.statement_cache.is_empty(),
        "预置失败：题面内存缓存为空"
    );
    assert!(
        !service.limits_cache.read().unwrap().is_empty(),
        "预置失败：limits 内存缓存为空"
    );

    service.clear_caches();

    assert!(service.statement_cache.is_empty(), "题面内存缓存应被清空");
    assert!(
        service.limits_cache.read().unwrap().is_empty(),
        "limits 内存缓存应被清空"
    );
    assert!(
        !statement_disk.exists(),
        "题面磁盘缓存应被同步清空（用户点了按钮就该等到真清完）"
    );
    assert!(!limits_disk.exists(), "limits 磁盘缓存应被同步清空");
    assert_eq!(
        call_count(&calls),
        calls_before,
        "清空缓存不得顺带发请求（补拉时机由调用方决定）"
    );

    // 清完再取必须真的回源 —— 否则「清了但没生效」
    block_on(service.load_problem_limits("1", &ids(&["A"]))).expect("清空后 limits 失败");
    assert_eq!(
        call_count(&calls),
        calls_before + 1,
        "清空后下一次查询必须回源"
    );
}

#[test]
fn clear_caches_without_any_disk_cache_is_not_an_error() {
    // 从未缓存过（目录不存在）时清空必须正常返回：remove_all 会返回 NotFound，
    // 不该被当成失败（与 OJSwitched 的存在性守卫同款）
    let (service, _calls, dir) = make_service("clear-caches-empty", Vec::new());

    assert!(
        !dir.join("cache").exists(),
        "前置条件：尚未产生任何磁盘缓存"
    );
    service.clear_caches();
}

// ── 事件发布契约（事实通知，不是命令）──

/// `open_problem` 发布 `ProblemOpened`（供审计 / 插件 / 前端其他页面响应）。
///
/// 题面内容**不进入事件**：事件只是「某题已被打开」的事实，正文由 IPC 返回值承载。
#[test]
fn open_problem_publishes_problem_opened_without_statement() {
    let dir = TempDir::named("hinina-test-problem-opened-event");
    let calls = Arc::new(AtomicUsize::new(0));
    let provider = Arc::new(StubProblemProvider {
        calls: Arc::clone(&calls),
        failing: Vec::new(),
        fail_all: false,
    });
    let registry: Arc<dyn ProviderRegistry> = Arc::new(ProviderRegistryImpl::new(OjId::new("HOJ")));
    registry.register(
        OjId::new("HOJ"),
        ProviderSet {
            problem: Some(Arc::clone(&provider) as Arc<dyn ProblemProvider>),
            ..Default::default()
        },
    );
    let bus = Arc::new(CoreEventBus::new());
    let service = ProblemService::new(
        registry,
        Arc::clone(&bus),
        Arc::new(Storage::new(dir.to_path_buf())),
    );
    let mut rx = bus.subscribe();

    block_on(service.open_problem("1011", "A", false)).expect("打开题目失败");

    let event = rx.try_recv().expect("应发布 ProblemOpened");
    assert_eq!(
        event,
        CoreEvent::ProblemOpened {
            contest_id: "1011".into(),
            problem_id: "A".into(),
        }
    );
    // 载荷只带 ID：题面（标题/样例/正文）绝不进入事件流
    let debug = format!("{event:?}");
    assert!(!debug.contains("Sample"), "{debug}");
    assert!(!debug.contains("title"), "{debug}");
}
