use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::core::error::{AppError, AppResult};
use crate::core::provider::auth::AuthProvider;
use crate::core::provider::contest::ContestProvider;
use crate::core::provider::oj_type::OJType;
use crate::core::provider::problem::ProblemProvider;
use crate::core::provider::registry::ProviderRegistry;
use crate::core::provider::submission::SubmissionProvider;

/// ProviderRegistry 的默认实现
pub struct ProviderRegistryImpl {
    auth_providers: RwLock<HashMap<OJType, Arc<dyn AuthProvider>>>,
    contest_providers: RwLock<HashMap<OJType, Arc<dyn ContestProvider>>>,
    problem_providers: RwLock<HashMap<OJType, Arc<dyn ProblemProvider>>>,
    submission_providers: RwLock<HashMap<OJType, Arc<dyn SubmissionProvider>>>,
    current: RwLock<OJType>,
}

impl ProviderRegistryImpl {
    pub fn new(default_oj: OJType) -> Self {
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
    fn register_auth(&self, oj_type: OJType, provider: Arc<dyn AuthProvider>) {
        if let Ok(mut map) = self.auth_providers.write() {
            map.insert(oj_type, provider);
        }
    }

    fn register_contest(&self, oj_type: OJType, provider: Arc<dyn ContestProvider>) {
        if let Ok(mut map) = self.contest_providers.write() {
            map.insert(oj_type, provider);
        }
    }

    fn register_problem(&self, oj_type: OJType, provider: Arc<dyn ProblemProvider>) {
        if let Ok(mut map) = self.problem_providers.write() {
            map.insert(oj_type, provider);
        }
    }

    fn register_submission(&self, oj_type: OJType, provider: Arc<dyn SubmissionProvider>) {
        if let Ok(mut map) = self.submission_providers.write() {
            map.insert(oj_type, provider);
        }
    }

    fn get_auth(&self, oj_type: &OJType) -> AppResult<Arc<dyn AuthProvider>> {
        let map = self.auth_providers.read().map_err(|e| {
            AppError::Unknown(format!("AuthProvider lock poisoned: {}", e))
        })?;
        map.get(oj_type)
            .cloned()
            .ok_or_else(|| AppError::ProviderNotFound(format!("AuthProvider for {:?}", oj_type)))
    }

    fn get_contest(&self, oj_type: &OJType) -> AppResult<Arc<dyn ContestProvider>> {
        let map = self.contest_providers.read().map_err(|e| {
            AppError::Unknown(format!("ContestProvider lock poisoned: {}", e))
        })?;
        map.get(oj_type)
            .cloned()
            .ok_or_else(|| AppError::ProviderNotFound(format!("ContestProvider for {:?}", oj_type)))
    }

    fn get_problem(&self, oj_type: &OJType) -> AppResult<Arc<dyn ProblemProvider>> {
        let map = self.problem_providers.read().map_err(|e| {
            AppError::Unknown(format!("ProblemProvider lock poisoned: {}", e))
        })?;
        map.get(oj_type)
            .cloned()
            .ok_or_else(|| AppError::ProviderNotFound(format!("ProblemProvider for {:?}", oj_type)))
    }

    fn get_submission(&self, oj_type: &OJType) -> AppResult<Arc<dyn SubmissionProvider>> {
        let map = self.submission_providers.read().map_err(|e| {
            AppError::Unknown(format!("SubmissionProvider lock poisoned: {}", e))
        })?;
        map.get(oj_type)
            .cloned()
            .ok_or_else(|| AppError::ProviderNotFound(format!("SubmissionProvider for {:?}", oj_type)))
    }

    fn current_oj(&self) -> OJType {
        self.current
            .read()
            .map(|oj| oj.clone())
            .unwrap_or(OJType::HOJ)
    }

    fn set_current_oj(&self, oj_type: OJType) {
        if let Ok(mut current) = self.current.write() {
            *current = oj_type;
        }
    }

    fn list_available(&self) -> Vec<OJType> {
        self.auth_providers
            .read()
            .map(|map| map.keys().cloned().collect())
            .unwrap_or_default()
    }
}
