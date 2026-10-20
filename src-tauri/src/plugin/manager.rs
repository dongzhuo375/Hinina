//! @plugin-module
//! @layer plugin/manager
//! @version 2
//! @stability stable
//! Minimal lifecycle: serialized start/stop/reload, isolated engine workers, generic Resync.
use super::{
    api::*,
    execution_api::{ExecutionApi, NotificationOutput},
    host::{
        manifest::PluginPermission,
        plugin_host::{PluginHost, PluginHostError},
    },
    query::{QueryBudget, QuerySource, ScopedQueries},
    runtime::quickjs::QuickJsRuntime,
};
use crate::plugin::registry::PluginRepository;
use crate::{
    core::access::AccessRevision,
    infra::{
        fs_plugin_repo::FsPluginRepository, plugin_http::PluginHttp,
        plugin_storage::JsonPluginStorage,
    },
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio_util::sync::CancellationToken;

/// @plugin-api
/// @since 2.0
/// @stability stable
/// Stable diagnostic codes only, never source/stack/paths.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginStatus {
    pub id: String,
    pub state: &'static str,
    pub error: Option<PluginError>,
}
struct Worker {
    stop: CancellationToken,
    thread: std::thread::JoinHandle<()>,
    status: Arc<Mutex<PluginStatus>>,
}

/// @plugin-api
/// @since 2.0
/// @stability stable
/// AppContext holds only Send controls, never a JS engine or JS values.
pub struct PluginManager {
    base: PathBuf,
    host: Arc<PluginHost>,
    source: Arc<dyn QuerySource>,
    access: Arc<AccessRevision>,
    workers: Mutex<BTreeMap<String, Worker>>,
    statuses: Mutex<VecDeque<PluginStatus>>,
    operation: tokio::sync::Mutex<()>,
    output: Mutex<Option<Arc<dyn NotificationOutput>>>,
    query_budget: Arc<QueryBudget>,
}
impl PluginManager {
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    pub fn new(
        base: PathBuf,
        host: Arc<PluginHost>,
        source: Arc<dyn QuerySource>,
        access: Arc<AccessRevision>,
    ) -> Self {
        Self {
            base,
            host,
            source,
            access,
            workers: Mutex::default(),
            statuses: Mutex::default(),
            operation: Default::default(),
            output: Mutex::default(),
            query_budget: Arc::new(QueryBudget::new(120, 4)),
        }
    }
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    pub fn set_output(&self, output: Arc<dyn NotificationOutput>) {
        *self.output.lock().unwrap_or_else(|e| e.into_inner()) = Some(output);
    }
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    pub fn list(&self) -> PluginResult<Vec<PluginStatus>> {
        let config = FsPluginRepository::new(self.base.clone()).configuration()?;
        let mut statuses = {
            let mut history = self.statuses.lock().unwrap_or_else(|e| e.into_inner());
            history.retain(|status| config.plugins.contains_key(&status.id));
            history
                .iter()
                .map(|status| (status.id.clone(), status.clone()))
                .collect::<BTreeMap<_, _>>()
        };
        for (id, _) in config.plugins {
            statuses.entry(id.clone()).or_insert(PluginStatus {
                id,
                state: "stopped",
                error: None,
            });
        }
        for (id, worker) in self
            .workers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
        {
            statuses.insert(
                id.clone(),
                worker
                    .status
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone(),
            );
        }
        Ok(statuses.into_values().collect())
    }
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    pub async fn start(&self, id: &str) -> PluginResult<()> {
        let _guard = self.operation.lock().await;
        self.start_inner(id).await
    }
    async fn start_inner(&self, id: &str) -> PluginResult<()> {
        self.start_worker(id).await.inspect_err(|error| {
            // PluginError only contains stable, redacted diagnostics. Loading may fail
            // before a worker exists, so worker.exit alone cannot report these failures.
            tracing::warn!(plugin_id = if id.len() <= 128 { id } else { "<invalid>" },
                runtime = "js", operation = "worker.start", error = ?error,
                "Plugin start failed");
            if id.len() <= 128 {
                self.remember_status(PluginStatus {
                    id: id.into(),
                    state: "failed",
                    error: Some(error.clone()),
                });
            }
        })
    }
    fn remember_status(&self, status: PluginStatus) {
        let mut history = self.statuses.lock().unwrap_or_else(|e| e.into_inner());
        history.retain(|old| old.id != status.id);
        if history.len() == 128 {
            history.pop_front();
        }
        history.push_back(status);
    }
    async fn start_worker(&self, id: &str) -> PluginResult<()> {
        if self
            .workers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .is_some_and(|w| !w.thread.is_finished())
        {
            return Err(PluginError::Unavailable);
        }
        self.workers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id);
        let base = self.base.clone();
        let plugin_id = id.to_string();
        let loaded =
            tokio::task::spawn_blocking(move || FsPluginRepository::new(base).load(&plugin_id))
                .await
                .map_err(|_| PluginError::Unavailable)??;
        let output = self
            .output
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or(PluginError::Unavailable)?;
        let stop = CancellationToken::new();
        let authority = Arc::new(
            ScopedQueries::new(
                loaded.grant.clone(),
                self.source.clone(),
                self.access.clone(),
                stop.clone(),
            )
            .with_shared_budget(self.query_budget.clone()),
        );
        let storage = JsonPluginStorage::new(self.base.join("plugin-data"), authority.clone());
        let http = PluginHttp::new(authority.clone(), loaded.approval.network.clone())?;
        let api: Arc<dyn PluginApi> = Arc::new(ExecutionApi::new(authority, storage, http, output));
        let mut events = if loaded
            .grant
            .require(PluginPermission::EventsSubscribe)
            .is_ok()
        {
            Some(
                self.host
                    .subscribe(&loaded.grant)
                    .map_err(|error| match error {
                        PluginHostError::PermissionDenied => PluginError::PermissionDenied {
                            permission: PluginPermission::EventsSubscribe,
                        },
                        PluginHostError::AlreadySubscribed | PluginHostError::AlreadyStarted => {
                            PluginError::Unavailable
                        }
                    })?,
            )
        } else {
            None
        };
        let allowed = if events.is_some() {
            loaded
                .grant
                .manifest()
                .events
                .iter()
                .filter(|event| {
                    event
                        .required_permission()
                        .is_none_or(|p| loaded.grant.require(p).is_ok())
                })
                .map(|e| e.as_str().to_string())
                .chain(std::iter::once("resync.required".into()))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let status = Arc::new(Mutex::new(PluginStatus {
            id: id.into(),
            state: "starting",
            error: None,
        }));
        let worker_status = status.clone();
        let cancellation = stop.clone();
        let host = self.host.clone();
        let id_owned = id.to_string();
        let handle = tokio::runtime::Handle::current();
        let (ready, started) = tokio::sync::oneshot::channel();
        let thread = std::thread::Builder::new().name("hinina-plugin".into()).spawn(move || {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handle.block_on(async {
                let mut runtime = match QuickJsRuntime::new(cancellation.clone()).await { Ok(runtime) => runtime, Err(error) => { let _ = ready.send(Err(error.clone())); return Err(error); } };
                let result = runtime.load(&loaded.source, api, &loaded.approval.options, allowed).await;
                let _ = ready.send(result.clone());
                if let Err(error) = result { let intentional = cancellation.is_cancelled(); cancellation.cancel(); runtime.shutdown().await; return if intentional { Ok(()) } else { Err(error) }; }
                worker_status.lock().unwrap_or_else(|e| e.into_inner()).state = "running";
                let mut outcome = Ok(());
                if let Some(events) = events.as_mut() {
                    let mut periodic = tokio::time::interval(Duration::from_secs(loaded.approval.resync_interval_secs)); periodic.tick().await;
                    periodic.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                    loop {
                        tokio::select! { biased;
                            _ = cancellation.cancelled() => break,
                            _ = periodic.tick() => { if !host.request_resync(&id_owned) { outcome = Err(PluginError::Unavailable); break; } },
                            event = events.recv() => {
                                if events.is_closed() { outcome = Err(PluginError::Unavailable); break; }
                                let Some(event) = event else { break; };
                                if let Err(error) = runtime.deliver(&event).await { outcome = Err(error); break; }
                            }
                        }
                    }
                } else {
                    cancellation.cancelled().await;
                }
                let intentional = cancellation.is_cancelled();
                cancellation.cancel();
                runtime.shutdown().await;
                if intentional { Ok(()) } else { outcome }
            }))).unwrap_or(Err(PluginError::Unavailable));
            cancellation.cancel(); host.unsubscribe(&id_owned);
            let mut status = worker_status.lock().unwrap_or_else(|e| e.into_inner());
            status.state = if outcome.is_ok() { "stopped" } else { "failed" };
            status.error = outcome.err();
            tracing::info!(plugin_id=%id_owned,plugin_version=%loaded.grant.manifest().version,runtime="js",event="",operation="worker.exit",permission="",request_id="",state=status.state,"Plugin worker exited");
        }).map_err(|_| { self.host.unsubscribe(id); PluginError::Unavailable })?;
        self.workers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                id.into(),
                Worker {
                    stop,
                    thread,
                    status,
                },
            );
        tokio::time::timeout(Duration::from_secs(12), started)
            .await
            .map_err(|_| PluginError::Timeout)?
            .map_err(|_| PluginError::Unavailable)?
    }
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    pub async fn stop(&self, id: &str) -> PluginResult<()> {
        // Interrupt activation before waiting for the lifecycle serialization lock.
        if let Some(worker) = self
            .workers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
        {
            worker.stop.cancel();
        }
        self.host.unsubscribe(id);
        let _guard = self.operation.lock().await;
        self.stop_inner(id).await
    }
    async fn stop_inner(&self, id: &str) -> PluginResult<()> {
        {
            let workers = self.workers.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(worker) = workers.get(id) {
                worker.stop.cancel();
            }
        }
        self.host.unsubscribe(id);
        let start = std::time::Instant::now();
        loop {
            let done = self
                .workers
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(id)
                .is_none_or(|w| w.thread.is_finished());
            if done {
                break;
            }
            if start.elapsed() > Duration::from_secs(5) {
                return Err(PluginError::Timeout);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        if let Some(worker) = self
            .workers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id)
        {
            let _ = worker.thread.join();
            self.remember_status(
                worker
                    .status
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone(),
            );
        }
        Ok(())
    }
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    pub async fn reload(&self, id: &str) -> PluginResult<()> {
        let _guard = self.operation.lock().await;
        self.stop_inner(id).await?;
        self.start_inner(id).await
    }
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    pub fn resync(&self, id: &str) -> PluginResult<()> {
        if self.host.request_resync(id) {
            Ok(())
        } else {
            Err(PluginError::Unavailable)
        }
    }
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    pub async fn autostart(&self) -> PluginResult<()> {
        let base = self.base.clone();
        let config =
            tokio::task::spawn_blocking(move || FsPluginRepository::new(base).configuration())
                .await
                .map_err(|_| PluginError::Unavailable)??;
        for (id, approval) in config.plugins {
            if approval.enabled {
                // start_inner retains failures for both automatic and explicit starts.
                let _ = self.start(&id).await;
            }
        }
        Ok(())
    }
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    /// Synchronous revocation on exit; does not wait on external I/O or workspace persistence.
    pub fn cancel_all(&self) {
        for (id, worker) in self
            .workers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
        {
            worker.stop.cancel();
            self.host.unsubscribe(id);
        }
    }
}
impl Drop for PluginManager {
    fn drop(&mut self) {
        self.cancel_all();
    }
}

#[cfg(test)]
#[path = "tests/manager_history_tests.rs"]
mod history_tests;
