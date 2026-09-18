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

use std::sync::Arc;
use std::time::Duration;

use tracing::{debug, info, warn};

use crate::core::entity::submission::{
    JudgementResult, JudgementStatus, SubmissionCases, SubmissionDetail, SubmissionPage,
    SubmissionQuery,
};
use crate::core::error::AppResult;
use crate::core::event::app_event::{AppEvent, SubmissionEvent};
use crate::core::event::event_bus::EventBus;
use crate::core::provider::registry::ProviderRegistry;
use crate::infra::cache::TtlCache;

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
    event_bus: Arc<EventBus>,
    /// 终态提交详情缓存（**仅内存**：用户域数据不落盘）
    detail_cache: TtlCache<String, SubmissionDetail>,
    /// 终态测试点缓存（**仅内存**）
    cases_cache: TtlCache<String, SubmissionCases>,
    /// 「该提交已确认终态」标记（由详情查询填充）。
    ///
    /// 测试点接口**不返回状态**，无法自证可缓存；用本标记作为判据既不必为判定终态
    /// 多发一次详情请求，也不会把评测中的半截明细缓存下来（缓存半截明细会让
    /// 「评测完成后打开详情页」看到缺失的测试点）。
    terminal_marks: TtlCache<String, bool>,
}

impl SubmissionService {
    /// 创建 SubmissionService。
    pub fn new(registry: Arc<dyn ProviderRegistry>, event_bus: Arc<EventBus>) -> Self {
        Self {
            registry,
            event_bus,
            detail_cache: TtlCache::new(SUBMISSION_CACHE_TTL, SUBMISSION_DETAIL_CAPACITY),
            cases_cache: TtlCache::new(SUBMISSION_CACHE_TTL, SUBMISSION_CASES_CAPACITY),
            terminal_marks: TtlCache::new(SUBMISSION_CACHE_TTL, SUBMISSION_DETAIL_CAPACITY),
        }
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

    /// 提交代码到 OJ。
    ///
    /// 返回 submission_id 供后续轮询使用。
    /// 发布 `SubmissionEvent::Created`。
    pub async fn submit(
        &self,
        contest_id: &str,
        problem_id: &str,
        language: &str,
        source_code: &str,
    ) -> AppResult<String> {
        let oj_id = self.registry.current_oj();
        let provider = self.registry.get_submission(&oj_id)?;

        info!(contest_id = contest_id, problem_id = problem_id, language = language, "提交代码");
        let submission_id = provider
            .submit(contest_id, problem_id, language, source_code)
            .await
            .map_err(|e| {
                warn!(contest_id = contest_id, problem_id = problem_id, error = %e, "提交失败");
                e.context("提交失败")
            })?;

        debug!(submission_id = submission_id, "代码已提交");

        self.event_bus.publish(&AppEvent::Submission(
            SubmissionEvent::Created {
                submission_id: submission_id.clone(),
            },
        ));

        Ok(submission_id)
    }

    /// 单次查询评测结果。
    ///
    /// 轮询节拍与总超时由前端 `submissionStore` 的 createPoller 拥有，后端不循环、
    /// 不睡眠、不设 deadline：每次调用只发一次 `provider.get_judgement`。
    /// 终态发布 `SubmissionEvent::Judged`；非终态（Pending/Compiling/Running）
    /// 原样透传、不发事件，是否继续轮询由前端决定。
    pub async fn get_judgement(&self, submission_id: &str) -> AppResult<JudgementResult> {
        let oj_id = self.registry.current_oj();
        let provider = self.registry.get_submission(&oj_id)?;

        // 错误一律 context() 补环节名、变体穿透（Auth 变体是前端 sessionGuard 的判据）；
        // 瞬时抖动的容忍与重试同样由前端 poller 编排
        let result = provider
            .get_judgement(submission_id)
            .await
            .map_err(|e| {
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
            self.event_bus.publish(&AppEvent::Submission(
                SubmissionEvent::Judged {
                    submission_id: submission_id.to_string(),
                    result: result.clone(),
                },
            ));
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
        let oj_id = self.registry.current_oj();
        let provider = self.registry.get_submission(&oj_id)?;

        let page = provider
            .list_contest_submissions(query)
            .await
            .map_err(|e| {
                warn!(contest_id = query.contest_id, error = %e, "获取提交列表失败");
                e.context("获取提交列表失败")
            })?;

        debug!(contest_id = query.contest_id, count = page.records.len(), "提交列表已获取");
        Ok(page)
    }

    /// 查询提交详情（含源代码与错误信息）。
    ///
    /// **仅终态结果入缓存**：评测中的详情会变（状态、耗时、内存、错误信息），
    /// 缓存它等于让界面停在「评测中」。命中缓存即零请求。
    pub async fn get_submission_detail(&self, submit_id: &str) -> AppResult<SubmissionDetail> {
        if let Some(detail) = self.detail_cache.get(&submit_id.to_string()) {
            debug!(cache = "submission_detail", submit_id, hit = true, "命中提交详情缓存");
            return Ok(detail);
        }

        let oj_id = self.registry.current_oj();
        let provider = self.registry.get_submission(&oj_id)?;

        let detail = provider
            .get_submission_detail(submit_id)
            .await
            .map_err(|e| {
                warn!(submit_id = submit_id, error = %e, "获取提交详情失败");
                e.context("获取提交详情失败")
            })?;

        // 只缓存成功且已终结的结果（错误绝不入缓存）；同时留下终态标记供测试点缓存判定
        if detail.status.is_terminal() {
            self.terminal_marks.insert(submit_id.to_string(), true);
            self.detail_cache
                .insert(submit_id.to_string(), detail.clone());
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
        if let Some(cases) = self.cases_cache.get(&submit_id.to_string()) {
            debug!(cache = "submission_cases", submit_id, hit = true, "命中测试点缓存");
            return Ok(cases);
        }

        let oj_id = self.registry.current_oj();
        let provider = self.registry.get_submission(&oj_id)?;

        let cases = provider
            .get_submission_cases(submit_id)
            .await
            .map_err(|e| {
                warn!(submit_id = submit_id, error = %e, "获取测试点结果失败");
                e.context("获取测试点结果失败")
            })?;

        if self.terminal_marks.get(&submit_id.to_string()).is_some() {
            self.cases_cache.insert(submit_id.to_string(), cases.clone());
        }

        Ok(cases)
    }
}

#[cfg(test)]
#[path = "tests/submission_tests.rs"]
mod tests;
