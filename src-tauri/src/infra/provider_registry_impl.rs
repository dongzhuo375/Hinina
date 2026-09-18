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
    ///
    /// 锁中毒统一 `into_inner` 取回内部数据（注册表进程级单例，中毒即全局
    /// 异常；与其余方法的恢复策略一致，不做静默失败或二次报错）。
    fn capability<T, F>(&self, pick: F) -> AppResult<Arc<T>>
    where
        F: FnOnce(&ProviderSet) -> &Option<Arc<T>>,
        T: ?Sized,
    {
        let id = self.current_id();
        let guard = self
            .providers
            .read()
            .unwrap_or_else(|e| e.into_inner());
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
        let mut map = self.providers.write().unwrap_or_else(|e| e.into_inner());
        map.insert(oj_id, set);
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
        let mut current = self.current.write().unwrap_or_else(|e| e.into_inner());
        *current = oj_id;
    }

    fn list_available(&self) -> Vec<OjId> {
        self.providers
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .keys()
            .cloned()
            .collect()
    }
}

#[cfg(test)]
#[path = "tests/provider_registry_impl_tests.rs"]
mod tests;
