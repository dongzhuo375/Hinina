use std::sync::Arc;

use crate::core::error::AppResult;
use crate::core::provider::auth::AuthProvider;
use crate::core::provider::contest::ContestProvider;
use crate::core::provider::oj_id::OjId;
use crate::core::provider::problem::ProblemProvider;
use crate::core::provider::submission::SubmissionProvider;

/// 一个 OJ 的 Provider 能力集合（注册侧的聚合值）。
///
/// 字段用 `Option`：新 Adapter 可以先只实现部分能力、逐步补齐
/// （开发手册「新 Adapter 可先只实现部分接口」的扩展路径）——
/// 缺失的能力在查询侧返回 `ProviderNotFound`，而不是让注册整个失败。
#[derive(Default)]
pub struct ProviderSet {
    pub auth: Option<Arc<dyn AuthProvider>>,
    pub contest: Option<Arc<dyn ContestProvider>>,
    pub problem: Option<Arc<dyn ProblemProvider>>,
    pub submission: Option<Arc<dyn SubmissionProvider>>,
}

impl ProviderSet {
    /// 四项能力齐备的集合（内建 OJ 通常一步到位）。
    pub fn full(
        auth: Arc<dyn AuthProvider>,
        contest: Arc<dyn ContestProvider>,
        problem: Arc<dyn ProblemProvider>,
        submission: Arc<dyn SubmissionProvider>,
    ) -> Self {
        Self {
            auth: Some(auth),
            contest: Some(contest),
            problem: Some(problem),
            submission: Some(submission),
        }
    }
}

/// Provider 注册中心。
///
/// 管理所有 OJ 的 Provider 实例，支持注册、按能力查询与 OJ 切换。
/// OJ 身份是数据（[`OjId`]）：接入新 OJ 不需要修改本文件。
///
/// **注册侧聚合、查询侧按能力**：聚合值（[`ProviderSet`]）只用于一次性
/// 注册/构造；查询保持 `current_auth()` 这类单项能力方法 —— 若提供返回
/// 聚合体的 `current()`，Service 将拿到它不需要的三个能力，接口隔离
/// 就从接口层面退化成了约定层面。
pub trait ProviderRegistry: Send + Sync {
    /// 注册一个 OJ 的能力集合（N 个 OJ = N 次调用，取代旧的 4×N 次单项注册）
    fn register(&self, oj_id: OjId, set: ProviderSet);

    /// 当前 OJ 的 AuthProvider（未注册 / 未提供该能力 → `ProviderNotFound`）
    fn current_auth(&self) -> AppResult<Arc<dyn AuthProvider>>;

    /// 当前 OJ 的 ContestProvider
    fn current_contest(&self) -> AppResult<Arc<dyn ContestProvider>>;

    /// 当前 OJ 的 ProblemProvider
    fn current_problem(&self) -> AppResult<Arc<dyn ProblemProvider>>;

    /// 当前 OJ 的 SubmissionProvider
    fn current_submission(&self) -> AppResult<Arc<dyn SubmissionProvider>>;

    /// 当前选中的 OJ
    fn current_id(&self) -> OjId;

    /// 切换当前 OJ
    fn set_current(&self, oj_id: OjId);

    /// 列出所有已注册的 OJ
    fn list_available(&self) -> Vec<OjId>;
}
