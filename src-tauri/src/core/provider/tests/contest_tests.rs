//! ContestProvider 默认实现的契约锁定。
//!
//! `ensure_contest_access` 的默认实现（当前 OJ 不支持该能力）必须是
//! `AppError::Contest` 变体：变体是前端 sessionGuard 的判定依据，
//! 「能力缺失」若被表达成 `Auth` 会让选手被误踢回登录页。

use super::*;

/// 仅继承默认实现的最小 provider：其余方法不应被调用。
struct DefaultOnlyProvider;

#[async_trait::async_trait]
impl ContestProvider for DefaultOnlyProvider {
    async fn list_contests(&self) -> AppResult<Vec<Contest>> {
        unreachable!("本用例不会调用 list_contests")
    }

    async fn get_contest(&self, _contest_id: &str) -> AppResult<Contest> {
        unreachable!("本用例不会调用 get_contest")
    }

    async fn list_contest_problems(&self, _contest_id: &str) -> AppResult<Vec<ContestProblem>> {
        unreachable!("本用例不会调用 list_contest_problems")
    }

    async fn get_contest_rank(
        &self,
        _contest_id: &str,
        _query: &RankQuery,
    ) -> AppResult<ContestRankPage> {
        unreachable!("本用例不会调用 get_contest_rank")
    }

    async fn list_announcements(
        &self,
        _contest_id: &str,
        _current_page: i64,
        _limit: i64,
    ) -> AppResult<AnnouncementPage> {
        unreachable!("本用例不会调用 list_announcements")
    }
}

#[test]
fn default_ensure_contest_access_is_contest_variant_with_hint() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("创建测试 runtime 失败");
    let err = rt
        .block_on(DefaultOnlyProvider.ensure_contest_access("1011", Some("pw")))
        .expect_err("默认实现应报不支持");
    assert!(
        matches!(err, AppError::Contest(ref m) if m.contains("不支持")),
        "能力缺失必须是 Contest 变体（不是 Auth，避免误踢会话），实际 {:?}",
        err
    );
}
