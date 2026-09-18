use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::core::error::{AppError, AppResult};
use crate::core::provider::auth::AuthProvider;
use crate::core::provider::contest::ContestProvider;
use crate::core::provider::oj_id::OjId;
use crate::core::provider::problem::ProblemProvider;
use crate::core::provider::registry::ProviderRegistry;
use crate::core::provider::submission::SubmissionProvider;

/// ProviderRegistry 的默认实现
///
/// 键为 [`OjId`]（数据身份）：注册哪些 OJ 是运行期数据，接入新 OJ 不需要
/// 修改本文件。
pub struct ProviderRegistryImpl {
    auth_providers: RwLock<HashMap<OjId, Arc<dyn AuthProvider>>>,
    contest_providers: RwLock<HashMap<OjId, Arc<dyn ContestProvider>>>,
    problem_providers: RwLock<HashMap<OjId, Arc<dyn ProblemProvider>>>,
    submission_providers: RwLock<HashMap<OjId, Arc<dyn SubmissionProvider>>>,
    current: RwLock<OjId>,
}

impl ProviderRegistryImpl {
    pub fn new(default_oj: OjId) -> Self {
        Self {
            auth_providers: RwLock::new(HashMap::new()),
            contest_providers: RwLock::new(HashMap::new()),
            problem_providers: RwLock::new(HashMap::new()),
            submission_providers: RwLock::new(HashMap::new()),
            current: RwLock::new(default_oj),
        }
    }
}

impl ProviderRegistry for ProviderRegistryImpl {
    fn register_auth(&self, oj_id: OjId, provider: Arc<dyn AuthProvider>) {
        if let Ok(mut map) = self.auth_providers.write() {
            map.insert(oj_id, provider);
        }
    }

    fn register_contest(&self, oj_id: OjId, provider: Arc<dyn ContestProvider>) {
        if let Ok(mut map) = self.contest_providers.write() {
            map.insert(oj_id, provider);
        }
    }

    fn register_problem(&self, oj_id: OjId, provider: Arc<dyn ProblemProvider>) {
        if let Ok(mut map) = self.problem_providers.write() {
            map.insert(oj_id, provider);
        }
    }

    fn register_submission(&self, oj_id: OjId, provider: Arc<dyn SubmissionProvider>) {
        if let Ok(mut map) = self.submission_providers.write() {
            map.insert(oj_id, provider);
        }
    }

    fn get_auth(&self, oj_id: &OjId) -> AppResult<Arc<dyn AuthProvider>> {
        let map = self.auth_providers.read().map_err(|e| {
            AppError::Unknown(format!("AuthProvider lock poisoned: {}", e))
        })?;
        map.get(oj_id)
            .cloned()
            .ok_or_else(|| AppError::ProviderNotFound(format!("AuthProvider for {}", oj_id)))
    }

    fn get_contest(&self, oj_id: &OjId) -> AppResult<Arc<dyn ContestProvider>> {
        let map = self.contest_providers.read().map_err(|e| {
            AppError::Unknown(format!("ContestProvider lock poisoned: {}", e))
        })?;
        map.get(oj_id)
            .cloned()
            .ok_or_else(|| AppError::ProviderNotFound(format!("ContestProvider for {}", oj_id)))
    }

    fn get_problem(&self, oj_id: &OjId) -> AppResult<Arc<dyn ProblemProvider>> {
        let map = self.problem_providers.read().map_err(|e| {
            AppError::Unknown(format!("ProblemProvider lock poisoned: {}", e))
        })?;
        map.get(oj_id)
            .cloned()
            .ok_or_else(|| AppError::ProviderNotFound(format!("ProblemProvider for {}", oj_id)))
    }

    fn get_submission(&self, oj_id: &OjId) -> AppResult<Arc<dyn SubmissionProvider>> {
        let map = self.submission_providers.read().map_err(|e| {
            AppError::Unknown(format!("SubmissionProvider lock poisoned: {}", e))
        })?;
        map.get(oj_id)
            .cloned()
            .ok_or_else(|| AppError::ProviderNotFound(format!("SubmissionProvider for {}", oj_id)))
    }

    fn current_oj(&self) -> OjId {
        // 锁中毒时取回内部数据（注册表进程级单例，中毒即全局异常，不静默回退）
        self.current
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn set_current_oj(&self, oj_id: OjId) {
        if let Ok(mut current) = self.current.write() {
            *current = oj_id;
        }
    }

    fn list_available(&self) -> Vec<OjId> {
        self.auth_providers
            .read()
            .map(|map| map.keys().cloned().collect())
            .unwrap_or_default()
    }
}
