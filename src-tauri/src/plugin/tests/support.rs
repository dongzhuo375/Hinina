use crate::core::entity::{contest::Contest, submission::*};
use crate::plugin::*;
use api::{PluginError, PluginResult};
use async_trait::async_trait;
use host::manifest::{PluginManifest, PluginPermission};
use query::{QueryScope, QuerySource, ScopedQueries};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub struct Source {
    pub scope: Mutex<QueryScope>,
    pub scope_error: Mutex<Option<PluginError>>,
    pub pages: Mutex<Vec<SubmissionHistoryPage>>,
    pub delay: bool,
    pub pending: bool,
    pub history_delay: Option<std::time::Duration>,
    pub history_started: tokio::sync::Notify,
    pub contest_calls: AtomicUsize,
    pub history_calls: AtomicUsize,
}
impl Default for Source {
    fn default() -> Self {
        Self {
            scope: Mutex::new(QueryScope {
                oj: "hoj".into(),
                endpoint: "https://oj.example.com".into(),
                user: Some(("user-1".into(), "alice".into())),
                contest: Some("contest".into()),
            }),
            scope_error: Mutex::new(None),
            pages: Mutex::new(vec![]),
            delay: false,
            pending: false,
            history_delay: None,
            history_started: tokio::sync::Notify::new(),
            contest_calls: AtomicUsize::new(0),
            history_calls: AtomicUsize::new(0),
        }
    }
}
#[async_trait]
impl QuerySource for Source {
    fn scope(&self) -> PluginResult<QueryScope> {
        if let Some(error) = self.scope_error.lock().unwrap().clone() {
            return Err(error);
        }
        Ok(self.scope.lock().unwrap().clone())
    }
    async fn contest(&self, _: &str) -> PluginResult<Contest> {
        self.contest_calls.fetch_add(1, Ordering::SeqCst);
        if self.pending {
            std::future::pending::<()>().await;
        }
        if self.delay {
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        }
        serde_json::from_value(
            serde_json::json!({"id":"contest","title":"Fixture","startTime":1,"endTime":2}),
        )
        .map_err(|_| PluginError::ProviderError)
    }
    async fn history(&self, _: &str, page: u32) -> PluginResult<SubmissionHistoryPage> {
        self.history_calls.fetch_add(1, Ordering::SeqCst);
        self.history_started.notify_one();
        if let Some(delay) = self.history_delay {
            tokio::time::sleep(delay).await;
        }
        if self.pending {
            std::future::pending::<()>().await;
        }
        if self.delay {
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        }
        Ok(self
            .pages
            .lock()
            .unwrap()
            .get(page as usize - 1)
            .cloned()
            .unwrap_or(SubmissionHistoryPage {
                records: vec![],
                total: None,
                has_more: false,
            }))
    }
}
pub fn manifest(id: &str) -> PluginManifest {
    serde_json::from_value(serde_json::json!({"id":id,"name":"Fixture","version":"1.0.0","manifestVersion":2,"apiVersion":2,"runtime":"js","entry":"index.js","permissions":["events.subscribe","submission.read","identity.read","contest.read","storage.read","storage.write","network.request","notification"],"events":["submission.judged"],"network":{"allowOrigins":["https://example.com"],"maxBodyBytes":65536,"timeoutMs":5000}})).unwrap()
}
pub fn authority(id: &str) -> Arc<ScopedQueries> {
    let m = manifest(id);
    Arc::new(ScopedQueries::new(
        permission::PluginGrant::authorize(&m, &m.permissions).unwrap(),
        Arc::new(Source::default()),
        Arc::default(),
        Default::default(),
    ))
}
pub fn record(id: &str) -> SubmissionRecord {
    SubmissionRecord {
        owner_id: Some("user-1".into()),
        submit_id: id.into(),
        pid: "problem".into(),
        display_pid: "1000".into(),
        title: String::new(),
        display_id: "A".into(),
        username: "alice".into(),
        submit_time: 1,
        status: JudgementStatus::Accepted,
        time_ms: 0,
        memory_kb: 0,
        score: None,
        length: 0,
        language: String::new(),
    }
}
pub fn grant_without(permission: PluginPermission) -> permission::PluginGrant {
    let m = manifest("test.denied");
    permission::PluginGrant::authorize(
        &m,
        &m.permissions
            .iter()
            .copied()
            .filter(|p| *p != permission)
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

pub fn make_manager(
    base: std::path::PathBuf,
    host: Arc<host::plugin_host::PluginHost>,
    source: Arc<dyn QuerySource>,
    access: Arc<crate::core::access::AccessRevision>,
) -> manager::PluginManager {
    manager::PluginManager::new(
        Arc::new(crate::infra::fs_plugin_repo::FsPluginRepository::new(
            base.clone(),
        )),
        Arc::new(crate::infra::plugin_api_factory::FsPluginApiFactory::new(
            base.join("plugin-data"),
        )),
        host,
        source,
        access,
    )
}
