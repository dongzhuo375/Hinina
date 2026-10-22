//! @plugin-module
//! @layer plugin/runtime
//! @version 2
//! @stability stable
//! Real single-module QuickJS adapter. One worker owns the engine; no loader or ambient I/O.
use crate::plugin::{
    api::{event::PluginEventEnvelope, *},
    execution_api::call_json,
    limits::{ACTIVATION_TIMEOUT, API_ERROR_GRACE, EVENT_TIMEOUT},
};
use rquickjs::{
    function::{Async, Func},
    AsyncContext, AsyncRuntime, Function, Module, Object, Persistent, Promise,
};
use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

/// @plugin-api
/// @since 2.0
/// @stability stable
/// Async watchdog plus the engine interrupt: synchronous JavaScript cannot evade the deadline.
pub struct QuickJsRuntime {
    context: AsyncContext,
    runtime: AsyncRuntime,
    deliver: Option<Persistent<Function<'static>>>,
    deadline: Arc<Mutex<Option<Instant>>>,
    stopped: CancellationToken,
    rejections: Arc<Mutex<Rejections>>,
    plugin_id: Option<String>,
}
impl Drop for QuickJsRuntime {
    fn drop(&mut self) {
        // rquickjs aborts the process when a Persistent outlives its runtime. `shutdown`
        // covers normal paths, but a worker panic unwinds without calling it, so the
        // persistent handler must be released before `context`/`runtime` are dropped.
        self.deliver.take();
    }
}
#[derive(Default)]
struct Rejections {
    pending: HashSet<u64>,
    overflow: bool,
}
impl Rejections {
    fn report(&mut self, plugin_id: Option<&str>) {
        if !self.pending.is_empty() || self.overflow {
            tracing::warn!(plugin_id=?plugin_id,runtime="js",operation="promise.rejection",
                category="unhandled_rejection",count=self.pending.len(),overflow=self.overflow,
                "Plugin Promise rejected without a handler at the end of the execution window");
        }
        self.pending.clear();
        self.overflow = false;
    }
}
impl QuickJsRuntime {
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    pub async fn new(stopped: CancellationToken) -> PluginResult<Self> {
        let runtime = AsyncRuntime::new().map_err(engine_error)?;
        runtime.set_memory_limit(16 * 1024 * 1024).await;
        runtime.set_max_stack_size(256 * 1024).await;
        let deadline: Arc<Mutex<Option<Instant>>> = Arc::default();
        let clock = deadline.clone();
        let cancellation = stopped.clone();
        let rejections = Arc::new(Mutex::new(Rejections::default()));
        let pending_rejections = rejections.clone();
        runtime
            .set_host_promise_rejection_tracker(Some(Box::new(move |_, promise, _, handled| {
                // Track identity only, never retain/format JS rejection reasons or grow without bound.
                let mut hash = std::collections::hash_map::DefaultHasher::new();
                promise.hash(&mut hash);
                let key = hash.finish();
                let mut state = pending_rejections.lock().unwrap_or_else(|e| e.into_inner());
                if handled {
                    state.pending.remove(&key);
                } else if state.pending.len() < 256 {
                    state.pending.insert(key);
                } else {
                    state.overflow = true;
                }
            })))
            .await;
        runtime
            .set_interrupt_handler(Some(Box::new(move || {
                cancellation.is_cancelled()
                    || clock
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .is_some_and(|end| Instant::now() >= end)
            })))
            .await;
        let context = AsyncContext::full(&runtime).await.map_err(engine_error)?;
        Ok(Self {
            context,
            runtime,
            deliver: None,
            deadline,
            stopped,
            rejections,
            plugin_id: None,
        })
    }
    /// @plugin-api
    /// @since 2.0
    /// @permission checked by API
    /// @stability stable
    pub async fn load(
        &mut self,
        source: &str,
        api: Arc<dyn PluginApi>,
        config: &serde_json::Value,
        allowed: Vec<String>,
    ) -> PluginResult<()> {
        if source.len() > 1024 * 1024 {
            return Err(PluginError::LimitExceeded);
        }
        self.plugin_id = api.audit_identity().map(|(id, _)| id.to_owned());
        let config = config.to_string();
        let allowed = serde_json::to_string(&allowed).map_err(|_| PluginError::InvalidInput)?;
        let stopped = self.stopped.clone();
        let execution_deadline = self.deadline.clone();
        let pending = Arc::new(tokio::sync::Semaphore::new(16));
        self.set_deadline(ACTIVATION_TIMEOUT);
        let operation = self.context.async_with(async |ctx| {
            let host = Func::from(Async(move |operation: String, input: String| {
                // Acquire before creating the Future: JS may enqueue many calls in one
                // synchronous turn, before the Rust futures get their first poll.
                let permit = pending.clone().try_acquire_owned().ok();
                let arguments = if permit.is_some() && operation.len() <= 64 && input.len() <= 140_000 {
                    Some((operation, input))
                } else { None };
                let api = api.clone(); let stopped = stopped.clone();
                // Anchor each call to this execution window, including calls made late
                // in a handler. The hard watchdog still bounds JS error handling itself.
                let call_deadline = execution_deadline.lock().unwrap_or_else(|e| e.into_inner())
                    .and_then(|end| end.checked_sub(API_ERROR_GRACE)).unwrap_or_else(Instant::now);
                async move {
                    let _permit = permit;
                    let Some((operation, input)) = arguments else { return "{\"ok\":false,\"error\":{\"code\":\"limit_exceeded\"}}".to_string(); };
                    tokio::select! { biased;
                        _ = stopped.cancelled() => "{\"ok\":false,\"error\":{\"code\":\"context_expired\"}}".to_string(),
                        _ = tokio::time::sleep_until(tokio::time::Instant::from_std(call_deadline)) => "{\"ok\":false,\"error\":{\"code\":\"timeout\"}}".to_string(),
                        result = call_json(api.as_ref(), operation, input) => result,
                    }
                }
            }));
            let factory: Function = ctx.eval(include_str!("bootstrap.js"))?;
            let bundle: Object = factory.call((host, ctx.json_parse(allowed)?))?;
            let deliver: Function = bundle.get("deliver")?;
            let js_api: Object = bundle.get("api")?;
            let config_fn: Function = bundle.get("config")?;
            let js_config: rquickjs::Value = config_fn.call((config,))?;
            let (module, evaluated) = Module::declare(ctx.clone(), "plugin-entry", source)?.eval()?;
            evaluated.into_future::<()>().await?;
            let activate: Function = module.get("default")?;
            let result: Promise = activate.call((js_api, js_config))?;
            result.into_future::<()>().await?;
            Ok::<_, rquickjs::Error>(Persistent::save(&ctx, deliver))
        });
        let result = tokio::select! { biased;
            _ = self.stopped.cancelled() => Err(PluginError::ContextExpired),
            result = tokio::time::timeout(ACTIVATION_TIMEOUT, operation) => result.map_err(|_| PluginError::Timeout).and_then(|value| value.map_err(engine_error)),
        };
        self.report_rejections();
        self.deliver = Some(result.map_err(|error| self.classify(error))?);
        self.set_deadline(EVENT_TIMEOUT);
        Ok(())
    }
    fn set_deadline(&self, duration: Duration) {
        *self.deadline.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now() + duration);
    }
    fn classify(&self, error: PluginError) -> PluginError {
        if self.stopped.is_cancelled() {
            PluginError::ContextExpired
        } else if self
            .deadline
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some_and(|end| Instant::now() >= end)
        {
            PluginError::Timeout
        } else {
            error
        }
    }
    fn report_rejections(&self) {
        self.rejections
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .report(self.plugin_id.as_deref());
    }
    /// @plugin-api
    /// @since 2.0
    /// @permission events.subscribe
    /// @stability stable
    pub async fn deliver(&mut self, envelope: &PluginEventEnvelope) -> PluginResult<()> {
        self.set_deadline(EVENT_TIMEOUT);
        let deliver = self
            .deliver
            .as_ref()
            .ok_or(PluginError::Unavailable)?
            .clone();
        let json = serde_json::to_string(envelope).map_err(|_| PluginError::InvalidInput)?;
        let operation = self.context.async_with(async |ctx| {
            let function = deliver.restore(&ctx)?;
            let promise: Promise = function.call((json,))?;
            promise.into_future::<()>().await
        });
        let result = tokio::select! { biased;
            _ = self.stopped.cancelled() => Err(PluginError::ContextExpired),
            result = tokio::time::timeout(EVENT_TIMEOUT, operation) => result.map_err(|_| PluginError::Timeout).and_then(|value| value.map_err(engine_error)),
        };
        self.report_rejections();
        result.map_err(|error| self.classify(error))
    }
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    pub async fn shutdown(&mut self) {
        self.stopped.cancel();
        self.deliver.take();
        self.runtime.run_gc().await;
        self.report_rejections();
    }
}
fn engine_error(error: rquickjs::Error) -> PluginError {
    match error {
        rquickjs::Error::Allocation => PluginError::LimitExceeded,
        rquickjs::Error::Exception => PluginError::ScriptError,
        _ => PluginError::Unavailable,
    }
}

#[cfg(test)]
#[path = "tests/quickjs_tests.rs"]
mod tests;

#[async_trait::async_trait(?Send)]
impl super::PluginRuntime for QuickJsRuntime {
    async fn activate(
        &mut self,
        source: &str,
        api: Arc<dyn PluginApi>,
        config: &serde_json::Value,
        allowed_events: Vec<String>,
    ) -> PluginResult<()> {
        self.load(source, api, config, allowed_events).await
    }
    async fn deliver_event(&mut self, envelope: &PluginEventEnvelope) -> PluginResult<()> {
        self.deliver(envelope).await
    }
    async fn shutdown(&mut self) -> PluginResult<()> {
        QuickJsRuntime::shutdown(self).await;
        Ok(())
    }
}
