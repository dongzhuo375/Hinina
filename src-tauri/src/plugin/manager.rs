//! @plugin-module
//! @layer plugin/manager
//! @version 2
//! @stability stable
//! Minimal lifecycle: serialized start/stop/reload, isolated engine workers, generic Resync.
use super::{
    api::{event::PluginEvent, *},
    execution_api::{NotificationOutput, PluginApiFactory},
    host::{
        manifest::PluginPermission,
        plugin_host::{PluginHost, PluginHostError},
    },
    limits::{SHARED_QUERIES_PER_MINUTE, SHARED_QUERY_CONCURRENCY, START_TIMEOUT, STOP_TIMEOUT},
    query::{QueryBudget, QuerySource, ScopedQueries},
    runtime::{quickjs::QuickJsRuntime, PluginRuntime},
};
use crate::core::access::AccessRevision;
use crate::plugin::registry::PluginRepository;
use serde::Serialize;
use std::{
    collections::{BTreeMap, VecDeque},
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
    reaping: bool,
}

/// @plugin-api
/// @since 2.0
/// @stability stable
/// AppContext holds only Send controls, never a JS engine or JS values.
pub struct PluginManager {
    repository: Arc<dyn PluginRepository>,
    factory: Arc<dyn PluginApiFactory>,
    host: Arc<PluginHost>,
    source: Arc<dyn QuerySource>,
    access: Arc<AccessRevision>,
    workers: Arc<Mutex<BTreeMap<String, Worker>>>,
    statuses: Arc<Mutex<VecDeque<PluginStatus>>>,
    operation: tokio::sync::Mutex<()>,
    output: Mutex<Option<Arc<dyn NotificationOutput>>>,
    query_budget: Arc<QueryBudget>,
    start_timeout: Duration,
    stop_timeout: Duration,
    resync_interval_override: Option<Duration>,
    /// Runs on the worker thread immediately after readiness is published, before the
    /// outcome is interpreted. Lets a test cancel the stop token at exactly the boundary a
    /// concurrent `stop()` can hit.
    #[cfg(test)]
    post_ready_hook: Option<Arc<dyn Fn() + Send + Sync>>,
}
impl PluginManager {
    #[cfg(test)]
    pub(crate) fn with_test_timing(
        mut self,
        start_timeout: Duration,
        resync_interval: Duration,
    ) -> Self {
        self.start_timeout = start_timeout;
        self.resync_interval_override = Some(resync_interval);
        self
    }
    #[cfg(test)]
    pub(crate) fn with_test_query_budget(mut self, count: u32) -> Self {
        self.query_budget = Arc::new(QueryBudget::new(count, SHARED_QUERY_CONCURRENCY));
        self
    }
    #[cfg(test)]
    pub(crate) fn with_test_stop_timeout(mut self, timeout: Duration) -> Self {
        self.stop_timeout = timeout;
        self
    }
    #[cfg(test)]
    pub(crate) fn with_test_post_ready_hook(mut self, hook: Arc<dyn Fn() + Send + Sync>) -> Self {
        self.post_ready_hook = Some(hook);
        self
    }
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    pub fn new(
        repository: Arc<dyn PluginRepository>,
        factory: Arc<dyn PluginApiFactory>,
        host: Arc<PluginHost>,
        source: Arc<dyn QuerySource>,
        access: Arc<AccessRevision>,
    ) -> Self {
        Self {
            repository,
            factory,
            host,
            source,
            access,
            workers: Arc::default(),
            statuses: Arc::default(),
            operation: Default::default(),
            output: Mutex::default(),
            query_budget: Arc::new(QueryBudget::new(
                SHARED_QUERIES_PER_MINUTE,
                SHARED_QUERY_CONCURRENCY,
            )),
            start_timeout: START_TIMEOUT,
            stop_timeout: STOP_TIMEOUT,
            resync_interval_override: None,
            #[cfg(test)]
            post_ready_hook: None,
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
        let config = self.repository.configuration()?;
        let mut statuses = {
            let mut history = self.statuses.lock().unwrap_or_else(|e| e.into_inner());
            history.retain(|status| {
                config.plugins.contains_key(&status.id)
                    || config.invalid_plugins.contains_key(&status.id)
            });
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
        for (id, error) in config.invalid_plugins {
            statuses.insert(
                id.clone(),
                PluginStatus {
                    id,
                    state: "failed",
                    error: Some(error),
                },
            );
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
        // A duplicate start is a rejected command, not a failed activation.
        if self
            .workers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .is_some_and(|w| !w.thread.is_finished())
        {
            return Err(PluginError::Unavailable);
        }
        self.start_worker(id).await.inspect_err(|error| {
            // PluginError only contains stable, redacted diagnostics. Loading may fail
            // before a worker exists, so worker.exit alone cannot report these failures.
            tracing::warn!(plugin_id = ?if id.len() <= 128 { id } else { "<invalid>" },
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
        remember_status(&self.statuses, status);
    }
    fn mark_stop_timeout(&self, id: &str) {
        if let Some(worker) = self
            .workers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
        {
            *worker.status.lock().unwrap_or_else(|e| e.into_inner()) = PluginStatus {
                id: id.into(),
                state: "failed",
                error: Some(PluginError::Timeout),
            };
        }
    }
    fn stop_timed_out(&self, id: &str) -> PluginResult<()> {
        self.mark_stop_timeout(id);
        self.reap_when_finished(id);
        Err(PluginError::Timeout)
    }
    fn reap_when_finished(&self, id: &str) {
        let workers = self.workers.clone();
        let statuses = self.statuses.clone();
        let id = id.to_owned();
        let expected = {
            let mut workers = workers.lock().unwrap_or_else(|e| e.into_inner());
            workers.get_mut(&id).and_then(|worker| {
                if worker.reaping {
                    return None;
                }
                worker.reaping = true;
                Some(worker.status.clone())
            })
        };
        let Some(expected) = expected else {
            return;
        };
        tokio::spawn(async move {
            let mut delay = Duration::from_millis(100);
            loop {
                let worker = {
                    let mut workers = workers.lock().unwrap_or_else(|e| e.into_inner());
                    match workers.get(&id) {
                        Some(worker) if Arc::ptr_eq(&worker.status, &expected) => {
                            if worker.thread.is_finished() {
                                // Publish the old generation's final history before allowing a replacement.
                                remember_status(
                                    &statuses,
                                    worker
                                        .status
                                        .lock()
                                        .unwrap_or_else(|e| e.into_inner())
                                        .clone(),
                                );
                                workers.remove(&id)
                            } else {
                                None
                            }
                        }
                        _ => return,
                    }
                };
                if let Some(worker) = worker {
                    let _ = worker.thread.join();
                    return;
                }
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(Duration::from_secs(2));
            }
        });
    }
    async fn start_worker(&self, id: &str) -> PluginResult<()> {
        {
            let mut workers = self.workers.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(worker) = workers.get(id) {
                if !worker.thread.is_finished() {
                    return Err(PluginError::Unavailable);
                }
                // Publish the finished generation before replacing it: start may race the
                // reaper, and the previous failure must survive in the status history.
                let status = worker
                    .status
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                remember_status(&self.statuses, status);
            }
            workers.remove(id);
        }
        let repository = self.repository.clone();
        let plugin_id = id.to_string();
        let loaded = tokio::task::spawn_blocking(move || repository.load(&plugin_id))
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
        let api = self.factory.create(authority, &loaded.approval, output)?;
        let subscribe = loaded
            .grant
            .require(PluginPermission::EventsSubscribe)
            .is_ok();
        let allowed = loaded
            .grant
            .allowed_events()
            .map(|event| event.as_str().to_owned())
            .collect::<Vec<_>>();
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
        let resync_interval = self
            .resync_interval_override
            .unwrap_or(Duration::from_secs(loaded.approval.resync_interval_secs));
        let (ready, started) = tokio::sync::oneshot::channel();
        #[cfg(test)]
        let post_ready = self.post_ready_hook.clone();
        let thread = std::thread::Builder::new().name("hinina-plugin".into()).spawn(move || {
            let outcome = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handle.block_on(async {
                let mut runtime: Box<dyn PluginRuntime> = match QuickJsRuntime::new(cancellation.clone()).await { Ok(runtime) => Box::new(runtime), Err(error) => { let _ = ready.send(Err(error.clone())); return Err(error); } };
                let mut result = runtime.activate(&loaded.source, api, &loaded.approval.options, allowed).await;
                let mut events = None;
                if result.is_ok() && !cancellation.is_cancelled() && subscribe {
                    match host.subscribe(&loaded.grant) {
                        Ok(receiver) => events = Some(receiver),
                        Err(error) => result = Err(match error {
                            PluginHostError::PermissionDenied => PluginError::PermissionDenied { permission: PluginPermission::EventsSubscribe },
                            _ => PluginError::Unavailable,
                        }),
                    }
                }
                // Snapshot before publishing the outcome: `stop()` cancels this token as soon as
                // the host observes readiness, so reading it afterwards would let a concurrent
                // stop relabel a real activation failure as a clean shutdown and erase the
                // failure from the status history the host just received.
                let interrupted = cancellation.is_cancelled();
                let _ = ready.send(result.clone());
                #[cfg(test)]
                if let Some(hook) = &post_ready { hook(); }
                if let Err(error) = result { cancellation.cancel(); let _ = runtime.shutdown().await; return if interrupted { Ok(()) } else { Err(error) }; }
                {
                    let mut status = worker_status.lock().unwrap_or_else(|e| e.into_inner());
                    if status.error.is_none() { status.state = "running"; }
                }
                let mut outcome = Ok(());
                if let Some(events) = events.as_mut() {
                    let mut periodic = tokio::time::interval(resync_interval); periodic.tick().await;
                    periodic.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                    loop {
                        tokio::select! { biased;
                            _ = cancellation.cancelled() => break,
                            _ = periodic.tick() => { if !host.request_resync(&id_owned) { outcome = Err(PluginError::Unavailable); break; } },
                            event = events.recv() => {
                                if events.is_closed() { outcome = Err(PluginError::Unavailable); break; }
                                let Some(event) = event else { break; };
                                // A resync notice must not itself re-trigger resync, or a failing
                                // resync handler would spawn an unbounded loop; the periodic tick
                                // still paces reconciliation for that case.
                                let is_resync = matches!(event.event, PluginEvent::ResyncRequired { .. });
                                match runtime.deliver_event(&event).await {
                                    Ok(delivery) => {
                                        // A rejected handler means the plugin's view may have
                                        // diverged; prompt one reconciliation so it does not run
                                        // stale until the next periodic resync.
                                        if delivery.failed_handlers > 0 && !is_resync && !host.request_resync(&id_owned) {
                                            outcome = Err(PluginError::Unavailable);
                                            break;
                                        }
                                    }
                                    Err(error) => { outcome = Err(error); break; }
                                }
                            }
                        }
                    }
                } else {
                    cancellation.cancelled().await;
                }
                let intentional = cancellation.is_cancelled();
                cancellation.cancel();
                let _ = runtime.shutdown().await;
                if intentional { Ok(()) } else { outcome }
            }))) {
                Ok(outcome) => outcome,
                Err(payload) => {
                    // Never format the payload: it may carry plugin-controlled text.
                    tracing::warn!(plugin_id=?id_owned,plugin_version=%loaded.grant.manifest().version,runtime="js",operation="worker.panic",category="panic",panic_kind=panic_kind(payload.as_ref()),"Plugin worker panicked");
                    Err(PluginError::Unavailable)
                }
            };
            cancellation.cancel(); host.unsubscribe(&id_owned);
            let status = {
                let mut status = worker_status.lock().unwrap_or_else(|e| e.into_inner());
                if status.error == Some(PluginError::Timeout) {
                    status.state = "failed";
                } else {
                    status.state = if outcome.is_ok() { "stopped" } else { "failed" };
                    status.error = outcome.err();
                }
                status.clone()
            };
            if status.error.is_some() {
                tracing::warn!(plugin_id=?id_owned,plugin_version=%loaded.grant.manifest().version,runtime="js",operation="worker.exit",state=status.state,error=?status.error,"Plugin worker failed");
            } else {
                tracing::info!(plugin_id=?id_owned,plugin_version=%loaded.grant.manifest().version,runtime="js",operation="worker.exit",state=status.state,"Plugin worker exited");
            }
        }).map_err(|_| { self.host.unsubscribe(id); PluginError::Unavailable })?;
        self.workers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                id.into(),
                Worker {
                    reaping: false,
                    stop,
                    thread,
                    status,
                },
            );
        match tokio::time::timeout(self.start_timeout, started).await {
            Ok(result) => result.map_err(|_| PluginError::Unavailable)?,
            Err(_) => {
                // Fail closed at the ready/deadline boundary: even a just-completed activation
                // is revoked if the caller did not observe readiness before its deadline.
                if let Some(worker) = self
                    .workers
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .get(id)
                {
                    *worker.status.lock().unwrap_or_else(|e| e.into_inner()) = PluginStatus {
                        id: id.into(),
                        state: "failed",
                        error: Some(PluginError::Timeout),
                    };
                }
                let cleanup = self.stop_inner(id).await;
                if let Err(error) = cleanup {
                    tracing::warn!(plugin_id=?id,operation="start.cleanup",error=?error,"Plugin cleanup pending");
                }
                Err(PluginError::Timeout)
            }
        }
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
            if start.elapsed() > self.stop_timeout {
                tracing::warn!(plugin_id=?id,operation="worker.stop",error=?PluginError::Timeout,"Plugin stop timed out");
                return self.stop_timed_out(id);
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
        let repository = self.repository.clone();
        let config = tokio::task::spawn_blocking(move || repository.configuration())
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
/// Panic payloads can carry plugin-controlled text; only the stable kind is logged.
fn panic_kind(payload: &(dyn std::any::Any + Send)) -> &'static str {
    if payload.is::<&str>() {
        "str"
    } else if payload.is::<String>() {
        "string"
    } else {
        "other"
    }
}
fn remember_status(history: &Mutex<VecDeque<PluginStatus>>, status: PluginStatus) {
    let mut history = history.lock().unwrap_or_else(|e| e.into_inner());
    history.retain(|old| old.id != status.id);
    if history.len() == 128 {
        history.pop_front();
    }
    history.push_back(status);
}
impl Drop for PluginManager {
    fn drop(&mut self) {
        self.cancel_all();
    }
}

#[cfg(test)]
#[path = "tests/manager_history_tests.rs"]
mod history_tests;

#[cfg(test)]
#[path = "tests/manager_tests.rs"]
mod tests;
