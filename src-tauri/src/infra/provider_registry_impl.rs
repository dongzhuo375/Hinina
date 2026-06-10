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
    fn register_auth(&mut self, oj_type: OJType, provider: Arc<dyn AuthProvider>) {
        self.auth_providers.get_mut().unwrap().insert(oj_type, provider);
    }

    fn register_contest(&mut self, oj_type: OJType, provider: Arc<dyn ContestProvider>) {
        self.contest_providers.get_mut().unwrap().insert(oj_type, provider);
    }

    fn register_problem(&mut self, oj_type: OJType, provider: Arc<dyn ProblemProvider>) {
        self.problem_providers.get_mut().unwrap().insert(oj_type, provider);
    }

    fn register_submission(&mut self, oj_type: OJType, provider: Arc<dyn SubmissionProvider>) {
        self.submission_providers.get_mut().unwrap().insert(oj_type, provider);
    }

    fn get_auth(&self, oj_type: &OJType) -> AppResult<Arc<dyn AuthProvider>> {
        self.auth_providers
            .read()
            .unwrap()
            .get(oj_type)
            .cloned()
            .ok_or_else(|| AppError::ProviderNotFound(format!("AuthProvider for {:?}", oj_type)))
    }

    fn get_contest(&self, oj_type: &OJType) -> AppResult<Arc<dyn ContestProvider>> {
        self.contest_providers
            .read()
            .unwrap()
            .get(oj_type)
            .cloned()
            .ok_or_else(|| AppError::ProviderNotFound(format!("ContestProvider for {:?}", oj_type)))
    }

    fn get_problem(&self, oj_type: &OJType) -> AppResult<Arc<dyn ProblemProvider>> {
        self.problem_providers
            .read()
            .unwrap()
            .get(oj_type)
            .cloned()
            .ok_or_else(|| AppError::ProviderNotFound(format!("ProblemProvider for {:?}", oj_type)))
    }

    fn get_submission(&self, oj_type: &OJType) -> AppResult<Arc<dyn SubmissionProvider>> {
        self.submission_providers
            .read()
            .unwrap()
            .get(oj_type)
            .cloned()
            .ok_or_else(|| AppError::ProviderNotFound(format!("SubmissionProvider for {:?}", oj_type)))
    }

    fn current_oj(&self) -> OJType {
        self.current.read().unwrap().clone()
    }

    fn set_current_oj(&self, oj_type: OJType) {
        *self.current.write().unwrap() = oj_type;
    }

    fn list_available(&self) -> Vec<OJType> {
        self.auth_providers.read().unwrap().keys().cloned().collect()
    }
}
