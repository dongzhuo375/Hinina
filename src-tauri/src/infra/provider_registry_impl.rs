use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::core::error::{AppError, AppResult};
use crate::core::provider::oj_id::OjId;
use crate::core::provider::registry::{ProviderRegistry, ProviderSet};

/// ProviderRegistry 的默认实现
///
/// 单表 `HashMap<OjId, ProviderSet>`（取代旧的 4 个能力分表）：
/// 注册哪些 OJ 是运行期数据，接入新 OJ 不需要修改本文件。
pub struct ProviderRegistryImpl {
    providers: RwLock<HashMap<OjId, ProviderSet>>,
    current: RwLock<OjId>,
}

impl ProviderRegistryImpl {
    pub fn new(default_oj: OjId) -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
            current: RwLock::new(default_oj),
        }
    }

    /// 按能力取当前 OJ 的 Provider（私有帮手，四个 current_xxx 共用）。
    fn capability<T, F>(&self, pick: F) -> AppResult<Arc<T>>
    where
        F: FnOnce(&ProviderSet) -> &Option<Arc<T>>,
        T: ?Sized,
    {
        let id = self.current_id();
        let guard = self
            .providers
            .read()
            .map_err(|e| AppError::Unknown(format!("ProviderRegistry lock poisoned: {}", e)))?;
        let set = guard
            .get(&id)
            .ok_or_else(|| AppError::ProviderNotFound(format!("OJ {} 未注册", id)))?;
        pick(set)
            .clone()
            .ok_or_else(|| AppError::ProviderNotFound(format!("OJ {} 未提供该能力", id)))
    }
}

impl ProviderRegistry for ProviderRegistryImpl {
    fn register(&self, oj_id: OjId, set: ProviderSet) {
        if let Ok(mut map) = self.providers.write() {
            map.insert(oj_id, set);
        }
    }

    fn current_auth(&self) -> AppResult<Arc<dyn crate::core::provider::auth::AuthProvider>> {
        self.capability(|set| &set.auth)
    }

    fn current_contest(&self) -> AppResult<Arc<dyn crate::core::provider::contest::ContestProvider>> {
        self.capability(|set| &set.contest)
    }

    fn current_problem(&self) -> AppResult<Arc<dyn crate::core::provider::problem::ProblemProvider>> {
        self.capability(|set| &set.problem)
    }

    fn current_submission(&self) -> AppResult<Arc<dyn crate::core::provider::submission::SubmissionProvider>> {
        self.capability(|set| &set.submission)
    }

    fn current_id(&self) -> OjId {
        // 锁中毒时取回内部数据（注册表进程级单例，中毒即全局异常，不静默回退）
        self.current
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn set_current(&self, oj_id: OjId) {
        if let Ok(mut current) = self.current.write() {
            *current = oj_id;
        }
    }

    fn list_available(&self) -> Vec<OjId> {
        self.providers
            .read()
            .map(|map| map.keys().cloned().collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
#[path = "tests/provider_registry_impl_tests.rs"]
mod tests;
