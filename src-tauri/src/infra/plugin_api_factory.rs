//! Concrete capability assembly; only the composition root selects filesystem/network implementations.
use super::{plugin_http::PluginHttp, plugin_storage::JsonPluginStorage};
use crate::plugin::{
    api::{PluginApi, PluginResult},
    execution_api::{ExecutionApi, NotificationOutput, PluginApiFactory},
    query::ScopedQueries,
    registry::PluginApproval,
};
use governor::{DefaultDirectRateLimiter, Quota, RateLimiter};
use std::{num::NonZeroU32, path::PathBuf, sync::Arc};

pub struct FsPluginApiFactory {
    data_root: PathBuf,
    storage_budget: Arc<DefaultDirectRateLimiter>,
}
impl FsPluginApiFactory {
    pub fn new(data_root: PathBuf) -> Self {
        Self {
            data_root,
            storage_budget: Arc::new(RateLimiter::direct(Quota::per_minute(
                NonZeroU32::new(240).unwrap(),
            ))),
        }
    }
}
impl PluginApiFactory for FsPluginApiFactory {
    fn create(
        &self,
        queries: Arc<ScopedQueries>,
        approval: &PluginApproval,
        output: Arc<dyn NotificationOutput>,
    ) -> PluginResult<Arc<dyn PluginApi>> {
        let storage = JsonPluginStorage::new(self.data_root.clone(), queries.clone())
            .with_shared_budget(self.storage_budget.clone());
        let http =
            PluginHttp::new(queries.clone(), approval.network.clone()).inspect_err(|_| {
                super::plugin_diagnostics::failure(
                    Some(&queries.grant.manifest().id),
                    "factory.create",
                    "capability_setup",
                );
            })?;
        Ok(Arc::new(ExecutionApi::new(queries, storage, http, output)))
    }
}
