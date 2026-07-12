// 题目服务：题目获取、本地缓存、题目切换时保留代码。
//
// 打开题目时自动创建/加载对应 Workspace，实现代码保留。
pub mod error;

use std::sync::Arc;

use tracing::{debug, info, warn};

use crate::core::entity::problem::Problem;
use crate::core::error::{AppError, AppResult};
use crate::core::event::app_event::{AppEvent, ProblemEvent};
use crate::core::event::event_bus::EventBus;
use crate::core::provider::registry::ProviderRegistry;
// WorkspaceManager 循环依赖通过运行时 Arc 注入解决
// （ProblemService 需要 WorkspaceManager, WorkspaceManager 可能切换到新的 problem）

/// 题目服务。
pub struct ProblemService {
    registry: Arc<dyn ProviderRegistry>,
    event_bus: Arc<EventBus>,
}

impl ProblemService {
    /// 创建 ProblemService。
    pub fn new(registry: Arc<dyn ProviderRegistry>, event_bus: Arc<EventBus>) -> Self {
        Self {
            registry,
            event_bus,
        }
    }

    /// 获取比赛下所有题目列表。
    pub async fn list_problems(&self, contest_id: &str) -> AppResult<Vec<Problem>> {
        let oj_type = self.registry.current_oj();
        let provider = self.registry.get_problem(&oj_type)?;

        debug!(contest_id = contest_id, "获取题目列表");
        let problems = provider.list_problems(contest_id).await.map_err(|e| {
            warn!(contest_id = contest_id, error = %e, "获取题目列表失败");
            AppError::Problem(format!("获取题目列表失败: {}", e))
        })?;

        debug!(contest_id = contest_id, count = problems.len(), "题目列表已获取");
        Ok(problems)
    }

    /// 打开题目：获取详情 + 发布 `ProblemEvent::Opened`。
    ///
    /// 调用方在收到此事件后应通过 WorkspaceManager 创建或切换工作区。
    /// 题目详情本身不依赖 WorkspaceManager，解耦关注点。
    pub async fn open_problem(
        &self,
        contest_id: &str,
        problem_id: &str,
    ) -> AppResult<Problem> {
        let oj_type = self.registry.current_oj();
        let provider = self.registry.get_problem(&oj_type)?;

        info!(contest_id = contest_id, problem_id = problem_id, "打开题目");
        let problem = provider
            .get_problem(contest_id, problem_id)
            .await
            .map_err(|e| {
                warn!(contest_id = contest_id, problem_id = problem_id, error = %e, "获取题目详情失败");
                AppError::Problem(format!("获取题目详情失败: {}", e))
            })?;

        self.event_bus
            .publish(&AppEvent::Problem(ProblemEvent::Opened {
                contest_id: contest_id.to_string(),
                problem_id: problem_id.to_string(),
            }));

        Ok(problem)
    }
}
