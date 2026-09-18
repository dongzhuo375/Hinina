use std::sync::Arc;

use crate::core::error::AppResult;
use crate::core::provider::auth::AuthProvider;
use crate::core::provider::contest::ContestProvider;
use crate::core::provider::oj_id::OjId;
use crate::core::provider::problem::ProblemProvider;
use crate::core::provider::submission::SubmissionProvider;

/// Provider 注册中心。
///
/// 管理所有 OJ 的 Provider 实例，支持注册、查找与 OJ 切换。
///
/// OJ 身份是数据（[`OjId`]），不是编译期枚举 —— 接入新 OJ 不需要修改本文件。
pub trait ProviderRegistry: Send + Sync {
    /// 注册 AuthProvider
    fn register_auth(&self, oj_id: OjId, provider: Arc<dyn AuthProvider>);

    /// 注册 ContestProvider
    fn register_contest(&self, oj_id: OjId, provider: Arc<dyn ContestProvider>);

    /// 注册 ProblemProvider
    fn register_problem(&self, oj_id: OjId, provider: Arc<dyn ProblemProvider>);

    /// 注册 SubmissionProvider
    fn register_submission(&self, oj_id: OjId, provider: Arc<dyn SubmissionProvider>);

    /// 获取 AuthProvider
    fn get_auth(&self, oj_id: &OjId) -> AppResult<Arc<dyn AuthProvider>>;

    /// 获取 ContestProvider
    fn get_contest(&self, oj_id: &OjId) -> AppResult<Arc<dyn ContestProvider>>;

    /// 获取 ProblemProvider
    fn get_problem(&self, oj_id: &OjId) -> AppResult<Arc<dyn ProblemProvider>>;

    /// 获取 SubmissionProvider
    fn get_submission(&self, oj_id: &OjId) -> AppResult<Arc<dyn SubmissionProvider>>;

    /// 获取当前选中的 OJ
    fn current_oj(&self) -> OjId;

    /// 切换当前 OJ
    fn set_current_oj(&self, oj_id: OjId);

    /// 列出所有已注册的 OJ
    fn list_available(&self) -> Vec<OjId>;
}
