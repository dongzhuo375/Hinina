// 题目服务：题目获取、limits 批量缓存、我的题目状态。
//
// 打开题目时自动创建/加载对应 Workspace，实现代码保留。
//
// **错误处理约定**：传播 Provider 错误一律用 `AppError::context()` 补环节名，
// 不得重新包装成 `AppError::Problem` —— 变体是前端 `sessionGuard` 判定会话失效的依据
// （见 `core/error.rs`）。`load_problem_limits` 同样遵守：401/403 原样上抛，
// 既不回退默认值，也不改写成 Problem 变体。
pub mod error;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use tracing::{debug, info, warn};

use crate::core::entity::problem::Problem;
use crate::core::entity::rank::ProblemLimits;
use crate::core::error::{AppError, AppResult};
use crate::core::event::app_event::{AppEvent, ProblemEvent, SystemEvent};
use crate::core::event::event_bus::EventBus;
use crate::core::event::event_category::EventCategory;
use crate::core::provider::registry::ProviderRegistry;
use crate::infra::cache::{JsonDiskCache, TtlCache};
use crate::infra::storage::Storage;
// WorkspaceManager 循环依赖通过运行时 Arc 注入解决
// （ProblemService 需要 WorkspaceManager, WorkspaceManager 可能切换到新的 problem）

/// 题目 limits 磁盘缓存目录。
/// limits 只能从题目详情接口取得且对同一题基本不变，跨重启复用可省掉整批详情请求。
const LIMITS_CACHE_DIR: &str = "cache/problem_limits";

/// limits 并发扇出上限：既要让首屏尽快补齐，也不能让单客户端瞬间打爆 OJ。
const LIMITS_CONCURRENCY: usize = 4;

/// 题面缓存 TTL。
///
/// 题面在比赛期间基本不变（改题罕见），但**并非永不变化**：管理员可能中途修正
/// 题面或样例。30 分钟是「切题来回/重进应用几乎总能命中」与「修正最多滞后半小时」
/// 之间的折中；想立刻看真值可在设置里临时关闭题面缓存
/// （`oj.cacheProblemStatement`）。
pub const PROBLEM_CACHE_TTL: Duration = Duration::from_secs(30 * 60);

/// 题面内存缓存容量上限（一场比赛题目量级远小于此，跨比赛也够用）。
const PROBLEM_CACHE_CAPACITY: usize = 200;

/// 题面磁盘缓存目录（跨重启复用；过期由 `fetchedAt` 判定并懒删除）。
const PROBLEM_CACHE_NAMESPACE: &str = "cache/problem_statement";

/// 题目服务。
pub struct ProblemService {
    registry: Arc<dyn ProviderRegistry>,
    event_bus: Arc<EventBus>,
    storage: Arc<Storage>,
    /// 内存缓存：`{oj}/{contest_id}` → (display_id → limits)
    ///
    /// 键同样带 OJ 维度（与题面/磁盘一致）：跨 OJ 不撞号，因此「切 OJ」的清理
    /// 只是让当前会话回到干净状态，而不是正确性的唯一依赖。
    limits_cache: Arc<RwLock<HashMap<String, HashMap<String, ProblemLimits>>>>,
    /// 题面缓存（内存 + 磁盘）：`{contest_id}/{display_id}` → Problem
    statement_cache: Arc<TtlCache<String, Problem>>,
    statement_disk: Arc<JsonDiskCache>,
}

impl ProblemService {
    /// 创建 ProblemService。
    pub fn new(
        registry: Arc<dyn ProviderRegistry>,
        event_bus: Arc<EventBus>,
        storage: Arc<Storage>,
    ) -> Self {
        let limits_cache = Arc::new(RwLock::new(HashMap::new()));
        let statement_cache = Arc::new(TtlCache::new(PROBLEM_CACHE_TTL, PROBLEM_CACHE_CAPACITY));
        let statement_disk = Arc::new(JsonDiskCache::new(Arc::clone(&storage), PROBLEM_CACHE_NAMESPACE));
        Self::subscribe_oj_switched(
            Arc::clone(&event_bus),
            Arc::clone(&limits_cache),
            Arc::clone(&statement_cache),
            Arc::clone(&statement_disk),
            Arc::clone(&storage),
        );
        Self {
            registry,
            event_bus,
            statement_cache,
            statement_disk,
            storage,
            limits_cache,
        }
    }

    /// 订阅 `OJSwitched`：清空题面与 limits 缓存。
    ///
    /// 缓存键自带 **OJ 维度**（`{oj}/{contest_id}/{display_id}`、
    /// `cache/problem_limits/{oj}/{cid}.json`），跨 OJ 撞号在结构上不可能 ——
    /// 「切 OJ」的清理只是空间回收，不承担正确性职责，故磁盘段可延迟执行
    /// （内存段仍是同步的：便宜，且让当前会话立刻回到干净状态）。
    fn subscribe_oj_switched(
        event_bus: Arc<EventBus>,
        limits_cache: Arc<RwLock<HashMap<String, HashMap<String, ProblemLimits>>>>,
        statement_cache: Arc<TtlCache<String, Problem>>,
        statement_disk: Arc<JsonDiskCache>,
        storage: Arc<Storage>,
    ) {
        // 同步段：纯内存清理（µs 级）
        event_bus.subscribe(
            EventCategory::System,
            Arc::new(move |event: &AppEvent| {
                let AppEvent::System(SystemEvent::OJSwitched { .. }) = event else {
                    return;
                };
                if let Ok(mut m) = limits_cache.write() {
                    m.clear();
                }
                statement_cache.clear();
                info!("OJ 已切换：清空题面与 limits 内存缓存");
            }),
        );

        // 延迟段：磁盘清理（I/O，仅空间回收 —— 键已带 OJ 维度，延迟不影响正确性）
        event_bus.subscribe_deferred(
            EventCategory::System,
            Arc::new(move |event: &AppEvent| {
                let AppEvent::System(SystemEvent::OJSwitched { .. }) = event else {
                    return;
                };
                let _ = statement_disk.clear_namespace();
                // 与 clear_namespace 同款存在性守卫：目录不存在（从未缓存过
                // limits 的常见情形）时 remove_all 会返回 NotFound，不告警
                if storage.exists(LIMITS_CACHE_DIR) {
                    if let Err(e) = storage.remove_all(LIMITS_CACHE_DIR) {
                        warn!(error = %e, "OJ 切换后清理 limits 磁盘缓存失败");
                    }
                }
                debug!("OJ 已切换：清理题面与 limits 磁盘缓存");
            }),
        );
    }

    /// 获取比赛下所有题目列表。
    pub async fn list_problems(&self, contest_id: &str) -> AppResult<Vec<Problem>> {
        let provider = self.registry.current_problem()?;

        debug!(contest_id = contest_id, "获取题目列表");
        let problems = provider.list_problems(contest_id).await.map_err(|e| {
            warn!(contest_id = contest_id, error = %e, "获取题目列表失败");
            e.context("获取题目列表失败")
        })?;

        debug!(contest_id = contest_id, count = problems.len(), "题目列表已获取");
        Ok(problems)
    }

    /// 打开题目：获取详情 + 发布 `ProblemEvent::Opened`。
    ///
    /// 调用方在收到此事件后应通过 WorkspaceManager 创建或切换工作区。
    /// 题目详情本身不依赖 WorkspaceManager，解耦关注点。
    ///
    /// **缓存策略**（`cache_enabled` 来自配置 `oj.cache_problem_statement`）：
    /// 开启时按 `{contest_id}/{display_id}` 走内存 → 磁盘 → 网络并回写两层；
    /// 关闭时直连服务端。无论命中与否都照常发布 `ProblemEvent::Opened`
    /// （WorkspaceManager 依赖它切换工作区）。
    ///
    /// 只缓存**成功结果**：Provider 错误（含 401/403）原样上抛，绝不入缓存。
    pub async fn open_problem(
        &self,
        contest_id: &str,
        problem_id: &str,
        cache_enabled: bool,
    ) -> AppResult<Problem> {
        let problem = self
            .load_problem_statement(contest_id, problem_id, cache_enabled)
            .await?;

        self.event_bus
            .publish(&AppEvent::Problem(ProblemEvent::Opened {
                contest_id: contest_id.to_string(),
                problem_id: problem_id.to_string(),
            }));

        Ok(problem)
    }

    /// 题面获取（含缓存编排）；`cache_enabled` 为 false 时完全直连服务端。
    async fn load_problem_statement(
        &self,
        contest_id: &str,
        problem_id: &str,
        cache_enabled: bool,
    ) -> AppResult<Problem> {
        let key = statement_key(self.registry.current_id().as_str(), contest_id, problem_id);

        if cache_enabled {
            if let Some(problem) = self.statement_cache.get(&key) {
                debug!(cache = "problem_statement", contest_id, problem_id, hit = true, "命中题面内存缓存");
                return Ok(problem);
            }
            if let Some(problem) = self
                .statement_disk
                .read::<Problem>(&key, PROBLEM_CACHE_TTL)
            {
                debug!(cache = "problem_statement", contest_id, problem_id, hit = true, "命中题面磁盘缓存");
                self.statement_cache.insert(key.clone(), problem.clone());
                return Ok(problem);
            }
        }

        let provider = self.registry.current_problem()?;

        info!(contest_id, problem_id, "打开题目");
        let problem = provider
            .get_problem(contest_id, problem_id)
            .await
            .map_err(|e| {
                warn!(contest_id, problem_id, error = %e, "获取题目详情失败");
                e.context("获取题目详情失败")
            })?;

        if cache_enabled {
            self.statement_cache.insert(key.clone(), problem.clone());
            self.statement_disk.write(&key, &problem);
        }

        Ok(problem)
    }

    /// 批量获取当前用户对指定题目的提交状态。
    ///
    /// 返回 map 的 key 为题目真实 ID（pid），value 为 `0=未提交 / 1=已AC / 2=尝试过`；
    /// 未出现在 map 中的题目视为未提交。
    pub async fn get_user_problem_status(
        &self,
        contest_id: &str,
        problem_ids: &[String],
    ) -> AppResult<HashMap<String, i32>> {
        if problem_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let provider = self.registry.current_problem()?;

        let statuses = provider
            .get_user_problem_status(contest_id, problem_ids)
            .await
            .map_err(|e| {
                warn!(contest_id = contest_id, error = %e, "获取用户题目状态失败");
                e.context("获取用户题目状态失败")
            })?;

        debug!(contest_id = contest_id, count = statuses.len(), "用户题目状态已获取");
        Ok(statuses)
    }

    /// 批量获取比赛题目的 limits（时间 ms / 内存 MB），带内存 + 磁盘双层缓存。
    ///
    /// 比赛题目列表接口不返回 limits，只能按题调用详情接口，因此：
    /// 内存未命中 → 读磁盘缓存 → 仍缺失的按 `LIMITS_CONCURRENCY` 分批并发补齐 → 回写两层缓存。
    ///
    /// 错误策略：单题失败只告警并跳过（该题在结果中缺失，前端显示占位而**不是**假默认值）；
    /// **全部失败**才上抛首个错误 —— 401/403 这类会话或权限问题必须让前端明确提示，
    /// 不能被静默吞成「拿不到 limits」（见 `HOJ-Problem-Limits-API.md` §9.5）。
    ///
    /// 返回顺序与入参 `display_ids` 一致。
    pub async fn load_problem_limits(
        &self,
        contest_id: &str,
        display_ids: &[String],
    ) -> AppResult<Vec<ProblemLimits>> {
        if display_ids.is_empty() {
            return Ok(Vec::new());
        }

        // 1. 内存缓存（键带 OJ 维度，与磁盘同源）
        let limits_key = self.limits_key(contest_id);
        let mut resolved: HashMap<String, ProblemLimits> = self
            .limits_cache
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(&limits_key)
            .cloned()
            .unwrap_or_default();

        // 2. 磁盘缓存（内存无该比赛任何记录时读一次）
        if resolved.is_empty() {
            resolved = self.read_limits_cache(contest_id);
        }

        // 3. 补齐缺失项
        let missing: Vec<String> = display_ids
            .iter()
            .filter(|id| !resolved.contains_key(*id))
            .cloned()
            .collect();

        if !missing.is_empty() {
            info!(contest_id = contest_id, missing = missing.len(), "补齐题目 limits");
            let (fetched, failures) = self.fetch_limits(contest_id, &missing).await;

            if fetched.is_empty() {
                if let Some((display_id, error)) = failures.into_iter().next() {
                    warn!(
                        contest_id = contest_id,
                        display_id = %display_id,
                        error = %error,
                        "题目 limits 全部获取失败"
                    );
                    return Err(error);
                }
            } else {
                for (display_id, error) in &failures {
                    warn!(
                        contest_id = contest_id,
                        display_id = %display_id,
                        error = %error,
                        "题目 limits 获取失败，该题保持缺失"
                    );
                }
                for item in &fetched {
                    resolved.insert(item.display_id.clone(), item.clone());
                }
                // 4. 回写内存与磁盘
                self.limits_cache
                    .write()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(limits_key, resolved.clone());
                self.write_limits_cache(contest_id, &resolved);
            }
        }

        Ok(display_ids
            .iter()
            .filter_map(|id| resolved.get(id).cloned())
            .collect())
    }

    // ── 内部方法 ──

    /// 分批并发获取缺失题目的 limits，返回 (成功项, 失败项)。
    async fn fetch_limits(
        &self,
        contest_id: &str,
        display_ids: &[String],
    ) -> (Vec<ProblemLimits>, Vec<(String, AppError)>) {
        let provider = match self.registry.current_problem() {
            Ok(p) => p,
            // Provider 不可用时整批失败，交由调用方上抛
            Err(e) => {
                let first = display_ids.first().cloned().unwrap_or_default();
                return (Vec::new(), vec![(first, e)]);
            }
        };

        let mut fetched = Vec::new();
        let mut failures = Vec::new();

        for chunk in display_ids.chunks(LIMITS_CONCURRENCY) {
            let mut set = tokio::task::JoinSet::new();
            for display_id in chunk {
                let provider = Arc::clone(&provider);
                let contest_id = contest_id.to_string();
                let display_id = display_id.clone();
                set.spawn(async move {
                    (display_id.clone(), provider.get_problem(&contest_id, &display_id).await)
                });
            }

            while let Some(joined) = set.join_next().await {
                match joined {
                    Ok((display_id, Ok(problem))) => fetched.push(ProblemLimits {
                        display_id,
                        time_limit: problem.time_limit,
                        memory_limit: problem.memory_limit,
                    }),
                    Ok((display_id, Err(e))) => failures.push((display_id, e)),
                    Err(e) => failures.push((
                        String::new(),
                        AppError::Unknown(format!("limits 任务 join 失败: {}", e)),
                    )),
                }
            }
        }

        (fetched, failures)
    }

    /// limits 内存缓存键：`{oj}/{cid}`（与磁盘路径同源作用域）。
    fn limits_key(&self, contest_id: &str) -> String {
        format!("{}/{}", self.registry.current_id(), contest_id)
    }

    /// limits 磁盘缓存路径：`cache/problem_limits/{oj}/{cid}.json`。
    ///
    /// 带 OJ 维度（与题面键同源理由）：跨 OJ 同 cid 不撞号，因此「切 OJ」的
    /// 磁盘清理只承担空间回收，可以延迟执行而不影响正确性。
    fn limits_cache_path(&self, contest_id: &str) -> String {
        format!(
            "{}/{}/{}.json",
            LIMITS_CACHE_DIR,
            self.registry.current_id(),
            contest_id
        )
    }

    /// 读取磁盘 limits 缓存；不存在或损坏时返回空（损坏文件会被后续回写覆盖）。
    fn read_limits_cache(&self, contest_id: &str) -> HashMap<String, ProblemLimits> {
        let path = self.limits_cache_path(contest_id);
        let Ok(raw) = self.storage.read_to_string(&path) else {
            return HashMap::new();
        };
        match serde_json::from_str::<HashMap<String, ProblemLimits>>(&raw) {
            Ok(map) => {
                debug!(contest_id = contest_id, count = map.len(), "命中题目 limits 磁盘缓存");
                map
            }
            Err(e) => {
                warn!(contest_id = contest_id, error = %e, "题目 limits 缓存解析失败，将重新获取");
                HashMap::new()
            }
        }
    }

    /// 回写磁盘 limits 缓存（失败只告警：缓存是优化，不影响正确性）。
    fn write_limits_cache(&self, contest_id: &str, map: &HashMap<String, ProblemLimits>) {
        if let Err(e) = self.storage.create_dir(LIMITS_CACHE_DIR) {
            warn!(error = %e, "创建 limits 缓存目录失败");
            return;
        }
        let path = self.limits_cache_path(contest_id);
        match serde_json::to_string_pretty(map) {
            Ok(json) => {
                if let Err(e) = self.storage.write_string(&path, &json) {
                    warn!(error = %e, path = %path, "写入 limits 缓存失败");
                }
            }
            Err(e) => warn!(error = %e, "limits 缓存序列化失败"),
        }
    }
}

/// 题面缓存键：`{oj}/{contest_id}/{display_id}`。
///
/// 三重作用域：**OJ**（跨 OJ 同 cid/pid 是不同服务端的不同数据）、比赛
/// （同一 `display_id` 在不同比赛是不同题目）、题目；同时让磁盘缓存按目录分层。
/// 键的安全性（拒绝 `..` / 绝对路径）由 `JsonDiskCache::key_path` 统一把关，
/// 这里只负责拼装。
fn statement_key(oj_id: &str, contest_id: &str, display_id: &str) -> String {
    format!("{}/{}/{}", oj_id, contest_id, display_id)
}

#[cfg(test)]
#[path = "tests/problem_tests.rs"]
mod tests;
