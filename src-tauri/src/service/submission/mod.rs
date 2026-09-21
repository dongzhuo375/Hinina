// 提交服务：代码提交、评测结果单次查询、提交列表 / 详情 / 测试点查询。
//
// 提交从当前 Workspace 获取代码。评测轮询的节拍与超时由前端 `submissionStore`
// 的 createPoller 拥有（配置 `oj.pollIntervalSecs` / `oj.pollTimeoutSecs` 经
// get_config 由前端消费）；后端只做单次查询与终态事件发布。
//
// **缓存策略**：终态提交的详情与测试点不可变，带仅内存的 TTL 缓存
// （`detail_cache` / `cases_cache`）—— 评测中的结果**永不缓存**；用户域数据
// **不落盘**，登出时由 `auth_cmd::logout` 编排 `clear_user_caches()` 清空。
//
// **错误处理约定**：传播 Provider 错误一律用 `AppError::context()` 补环节名，
// 不得重新包装成 `AppError::Submission` —— 变体是前端 `sessionGuard` 判定会话失效的依据
// （见 `core/error.rs`）。token 过期时提交若被改写成 Submission 变体，
// 选手只会看到一条错误文案而不会被带回登录页，反复重试也全部失败。
pub mod error;
pub mod snapshot;

use std::sync::Arc;
use std::time::Duration;

use tracing::{debug, info, warn};

use crate::core::entity::submission::{
    JudgementResult, JudgementStatus, SubmissionCases, SubmissionDetail, SubmissionPage,
    SubmissionQuery,
};
use crate::core::error::AppResult;
use crate::core::event::core_event::CoreEvent;
use crate::core::event::core_event_bus::CoreEventBus;
use crate::core::provider::registry::ProviderRegistry;
use crate::infra::cache::TtlCache;
use crate::infra::storage::Storage;

/// 终态提交详情/测试点的内存缓存 TTL。
///
/// 终态结果**不可变**，TTL 取长值（2 小时）只为让条目最终被回收 —— 真正的内存
/// 上界由容量上限保证（见下）。会话内反复进出详情页（列表 → 详情 → 返回 → 再进）
/// 是高频动作，每次都重拉纯属浪费。
const SUBMISSION_CACHE_TTL: Duration = Duration::from_secs(2 * 60 * 60);

/// 终态提交详情缓存容量上限。
const SUBMISSION_DETAIL_CAPACITY: usize = 200;

/// 终态测试点缓存容量上限（条目更大，故更小）。
const SUBMISSION_CASES_CAPACITY: usize = 100;

/// 提交服务。
pub struct SubmissionService {
    registry: Arc<dyn ProviderRegistry>,
    event_bus: Arc<CoreEventBus>,
    /// 终态提交详情缓存（**仅内存**：用户域数据不落盘）
    detail_cache: Arc<TtlCache<String, SubmissionDetail>>,
    /// 终态测试点缓存（**仅内存**）
    cases_cache: Arc<TtlCache<String, SubmissionCases>>,
    /// 「该提交已确认终态」标记（由详情查询填充）。
    ///
    /// 测试点接口**不返回状态**，无法自证可缓存；用本标记作为判据既不必为判定终态
    /// 多发一次详情请求，也不会把评测中的半截明细缓存下来（缓存半截明细会让
    /// 「评测完成后打开详情页」看到缺失的测试点）。
    terminal_marks: Arc<TtlCache<String, bool>>,
    /// 文件存储：用于提交源码快照（本地留档，OJ 不回吐代码时的兜底）
    storage: Arc<Storage>,
}

impl SubmissionService {
    /// 创建 SubmissionService。
    pub fn new(
        registry: Arc<dyn ProviderRegistry>,
        event_bus: Arc<CoreEventBus>,
        storage: Arc<Storage>,
    ) -> Self {
        let detail_cache = Arc::new(TtlCache::new(
            SUBMISSION_CACHE_TTL,
            SUBMISSION_DETAIL_CAPACITY,
        ));
        let cases_cache = Arc::new(TtlCache::new(
            SUBMISSION_CACHE_TTL,
            SUBMISSION_CASES_CAPACITY,
        ));
        let terminal_marks = Arc::new(TtlCache::new(
            SUBMISSION_CACHE_TTL,
            SUBMISSION_DETAIL_CAPACITY,
        ));
        Self {
            registry,
            event_bus,
            detail_cache,
            cases_cache,
            terminal_marks,
            storage,
        }
    }

    /// OJ 切换后的用户域缓存清理（**由 `switch_oj` 命令显式调用**，不订阅事件）。
    ///
    /// 缓存内是**旧 OJ 用户**的提交详情与测试点（含源代码），切换后不得复用
    /// —— 与登出清理同一语义（换 OJ 即换用户上下文）。
    ///
    /// 为什么显式：`switch_oj` 返回后新 OJ 的查询立刻可能进来，清缓存属于
    /// 切换正确性的一部分，不能依赖异步投递（消费者可能落后、可能不存在）。
    /// 三个缓存都在内存里，清理本身是 µs 级，无需后台任务。
    pub fn on_oj_switched(&self) {
        self.clear_user_caches();
        info!("OJ 已切换：清空提交详情/测试点缓存（用户域数据不得跨 OJ 复用）");
    }

    /// 清空用户域缓存（登出时由 `auth_cmd::logout` 编排调用）。
    ///
    /// 缓存里存的是**当前用户的提交详情与测试点**（含源代码），换账号后不得复用 ——
    /// 这也是它们只放内存、不落盘的原因。
    pub fn clear_user_caches(&self) {
        self.detail_cache.clear();
        self.cases_cache.clear();
        self.terminal_marks.clear();
        info!("已清空提交详情/测试点缓存");
    }

    /// 提交缓存键：`{oj}/{submit_id}`。
    ///
    /// `submit_id` 是**各 OJ 自增的资源号**，不同 OJ 必然重号 —— 键带 OJ 维度后，
    /// 跨 OJ 串号在结构上不可能（「切 OJ 时清缓存」因此只是让当前会话立刻干净，
    /// 而不是正确性的唯一依赖）。
    fn cache_key(&self, submit_id: &str) -> String {
        format!("{}/{}", self.registry.current_id(), submit_id)
    }

    /// 提交代码到 OJ。
    ///
    /// 返回 submission_id 供后续轮询使用。
    /// 发布 `CoreEvent::SubmissionCreated`。
    ///
    /// `problem_id` 与 `display_id` 同时下传（各 OJ 认的不是同一个标识，详见
    /// `SubmissionProvider::submit`）；成功后在本地留一份源码快照。
    pub async fn submit(
        &self,
        contest_id: &str,
        problem_id: &str,
        display_id: &str,
        language: &str,
        source_code: &str,
    ) -> AppResult<String> {
        let provider = self.registry.current_submission()?;

        info!(
            contest_id = contest_id,
            problem_id = problem_id,
            display_id = display_id,
            language = language,
            "提交代码"
        );
        let submission_id = provider
            .submit(contest_id, problem_id, display_id, language, source_code)
            .await
            .map_err(|e| {
                warn!(contest_id = contest_id, problem_id = problem_id, error = %e, "提交失败");
                e.context("提交失败")
            })?;

        debug!(submission_id = submission_id, "代码已提交");

        // 本地留档「当时提交的代码」：OJ 可能在比赛隐藏记录、codeShare=false、
        // 赛后回收等情形下不回吐代码，届时详情页回落到这份快照（best-effort）
        snapshot::write_snapshot(
            &self.storage,
            self.registry.current_id().as_str(),
            &submission_id,
            language,
            source_code,
        );

        self.event_bus.publish(CoreEvent::SubmissionCreated {
            submission_id: submission_id.clone(),
        });

        Ok(submission_id)
    }

    /// 单次查询评测结果。
    ///
    /// 轮询节拍与总超时由前端 `submissionStore` 的 createPoller 拥有，后端不循环、
    /// 不睡眠、不设 deadline：每次调用只发一次 `provider.get_judgement`。
    /// 终态发布 `CoreEvent::SubmissionJudged`（状态摘要）；非终态（Pending/Compiling/Running）
    /// 原样透传、不发事件，是否继续轮询由前端决定。
    pub async fn get_judgement(&self, submission_id: &str) -> AppResult<JudgementResult> {
        let provider = self.registry.current_submission()?;

        // 错误一律 context() 补环节名、变体穿透（Auth 变体是前端 sessionGuard 的判据）；
        // 瞬时抖动的容忍与重试同样由前端 poller 编排
        let result = provider.get_judgement(submission_id).await.map_err(|e| {
            warn!(submission_id = submission_id, error = %e, "评测查询失败");
            e.context("评测查询失败")
        })?;

        // 非终态判据与原轮询循环一致：Pending/Compiling/Running 三态之外即终态
        if matches!(
            result.status,
            JudgementStatus::Pending | JudgementStatus::Compiling | JudgementStatus::Running
        ) {
            debug!(
                submission_id = submission_id,
                status = ?result.status,
                "评测进行中"
            );
        } else {
            debug!(
                submission_id = submission_id,
                status = ?result.status,
                "评测完成"
            );
            self.event_bus.publish(CoreEvent::SubmissionJudged {
                submission_id: submission_id.to_string(),
                status: result.status.as_str().to_string(),
            });
        }

        Ok(result)
    }

    /// 查询比赛提交列表（分页）。
    ///
    /// **不做缓存**：提交状态随时在变（评测中 → 终态），必须由前端控制刷新节奏。
    pub async fn list_contest_submissions(
        &self,
        query: &SubmissionQuery,
    ) -> AppResult<SubmissionPage> {
        let provider = self.registry.current_submission()?;

        let page = provider
            .list_contest_submissions(query)
            .await
            .map_err(|e| {
                warn!(contest_id = query.contest_id, error = %e, "获取提交列表失败");
                e.context("获取提交列表失败")
            })?;

        debug!(
            contest_id = query.contest_id,
            count = page.records.len(),
            "提交列表已获取"
        );
        Ok(page)
    }

    /// 查询提交详情（含源代码与错误信息）。
    ///
    /// **仅终态结果入缓存**：评测中的详情会变（状态、耗时、内存、错误信息），
    /// 缓存它等于让界面停在「评测中」。命中缓存即零请求。
    pub async fn get_submission_detail(&self, submit_id: &str) -> AppResult<SubmissionDetail> {
        let key = self.cache_key(submit_id);
        if let Some(detail) = self.detail_cache.get(&key) {
            debug!(
                cache = "submission_detail",
                submit_id,
                hit = true,
                "命中提交详情缓存"
            );
            return Ok(detail);
        }

        let provider = self.registry.current_submission()?;

        let mut detail = provider
            .get_submission_detail(submit_id)
            .await
            .map_err(|e| {
                warn!(submit_id = submit_id, error = %e, "获取提交详情失败");
                e.context("获取提交详情失败")
            })?;

        // OJ 未回吐代码（比赛隐藏记录 / codeShare=false / 赛后回收）时回落到本地快照。
        // 这是「当时的代码」唯一的本地来源 —— 没有它，选手只能看到一个空代码框
        if detail.code.trim().is_empty() {
            if let Some(code) = snapshot::read_snapshot(
                &self.storage,
                self.registry.current_id().as_str(),
                submit_id,
                &detail.language,
            ) {
                debug!(submit_id = submit_id, "OJ 未返回代码，已回落到本地快照");
                detail.code = code;
            }
        }

        // 只缓存成功且已终结的结果（错误绝不入缓存）；同时留下终态标记供测试点缓存判定
        if detail.status.is_terminal() {
            self.terminal_marks.insert(key.clone(), true);
            self.detail_cache.insert(key, detail.clone());
        }

        Ok(detail)
    }

    /// 查询提交的全部测试点结果。
    ///
    /// **仅终态结果入缓存**：评测中测试点会逐个产生，缓存会让明细停在半截。
    /// 终态判据取 `terminal_marks`（由详情查询填充）—— 详情页流程天然先拉详情
    /// 再拉测试点；控制台条的失败测试点提示直接拉测试点、没有详情上下文，
    /// 此时**不缓存**（保持与改造前一致的请求数，而不是为判定终态多发一次详情请求）。
    pub async fn get_submission_cases(&self, submit_id: &str) -> AppResult<SubmissionCases> {
        let key = self.cache_key(submit_id);
        if let Some(cases) = self.cases_cache.get(&key) {
            debug!(
                cache = "submission_cases",
                submit_id,
                hit = true,
                "命中测试点缓存"
            );
            return Ok(cases);
        }

        let provider = self.registry.current_submission()?;

        let cases = provider
            .get_submission_cases(submit_id)
            .await
            .map_err(|e| {
                warn!(submit_id = submit_id, error = %e, "获取测试点结果失败");
                e.context("获取测试点结果失败")
            })?;

        if self.terminal_marks.get(&key).is_some() {
            self.cases_cache.insert(key, cases.clone());
        }

        Ok(cases)
    }
}

#[cfg(test)]
#[path = "tests/submission_tests.rs"]
mod tests;
