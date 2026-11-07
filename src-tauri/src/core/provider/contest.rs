use async_trait::async_trait;

use crate::core::entity::announcement::AnnouncementPage;
use crate::core::entity::contest::{Contest, ContestProblem};
use crate::core::entity::rank::{ContestRankPage, RankQuery};
use crate::core::error::{AppError, AppResult};

/// 比赛 Provider：获取比赛列表与详情
#[async_trait]
pub trait ContestProvider: Send + Sync {
    /// 获取比赛列表
    async fn list_contests(&self) -> AppResult<Vec<Contest>>;

    /// 获取比赛详情
    async fn get_contest(&self, contest_id: &str) -> AppResult<Contest>;

    /// 获取比赛题目列表（仅摘要，不含完整题面）
    async fn list_contest_problems(&self, contest_id: &str) -> AppResult<Vec<ContestProblem>>;

    /// 获取比赛排行榜（分页）。
    ///
    /// 实现约定：
    /// - 返回的行可能包含服务端前置的「当前用户/关注用户」副本，调用方渲染前需按 `uid` 去重；
    /// - `total` 含这些前置条目，**不能**当作真实参赛人数；
    /// - 榜单为服务端实时计算，本方法不做缓存，轮询节奏由调用方控制（建议 ≥10s 且加抖动错峰）。
    async fn get_contest_rank(
        &self,
        contest_id: &str,
        query: &RankQuery,
    ) -> AppResult<ContestRankPage>;

    /// 获取比赛公告（分页）。
    ///
    /// 公告可能包含裁判组临场发布的规则变更，实现方**不得缓存**，
    /// 刷新节奏由调用方控制。
    async fn list_announcements(
        &self,
        contest_id: &str,
        current_page: i64,
        limit: i64,
    ) -> AppResult<AnnouncementPage>;

    /// 确保当前会话可访问指定比赛（私有赛/需报名赛），必要时用凭证报名进入。
    ///
    /// 「凭证进入比赛」是各 OJ 的公因数：HOJ 为比赛密码（register-contest），
    /// 其它 OJ 可为邀请码（如 Hydro 的 attend）——凭证语义由适配器自行解释。
    /// 实现约定：
    /// - 已可访问时幂等返回 `Ok(())`（报名/加入通常是账号级持久状态，可先查后报）；
    /// - 需要凭证而未提供、或凭证被服务端拒绝时返回 `AppError::Contest`，
    ///   消息应能引导用户在设置页补填/更正；
    /// - 会话失效时返回 `AppError::Auth`（变体是前端 sessionGuard 的判定依据）。
    ///
    /// 默认实现返回不支持错误：当前仅 HOJ 实现该能力。Hydro 的比赛可见性由
    /// 域权限/报名表达，Hinina 侧其 `auth` 恒映射为 0，不会触发本方法。
    async fn ensure_contest_access(
        &self,
        _contest_id: &str,
        _credential: Option<&str>,
    ) -> AppResult<()> {
        Err(AppError::Contest(
            "当前 OJ 不支持凭密码/凭证进入私有赛".to_string(),
        ))
    }

    /// 判断题目列表的业务失败是否属于「需凭证进入」类（错误驱动进入流程的
    /// 精确触发判据）。
    ///
    /// 「需凭证的失败以 `Contest` 变体表达」只是**必要条件，不充分**——
    /// 「比赛还未开始」「比赛不存在」等同样落在 `Contest` 变体。是否属需凭证
    /// 只有适配器自己能可靠解读（各 OJ 的业务消息措辞不同），故收在适配器侧：
    /// - 返回 true：调用方可走凭证进入流程；
    /// - 返回 false：错误原样上抛（**安全降级方向**——漏判的代价是进入流程
    ///   不触发，而不是给用户看错的引导）。
    ///
    /// 默认实现恒 false：未声明该能力的 OJ 不触发进入流程。
    fn is_credential_rejection(&self, _error: &AppError) -> bool {
        false
    }
}

#[cfg(test)]
#[path = "tests/contest_tests.rs"]
mod tests;
