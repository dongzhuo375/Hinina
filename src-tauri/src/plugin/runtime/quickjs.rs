//! @plugin-module
//! @layer plugin/runtime
//! @version 2
//! @stability stable
//! Real single-module QuickJS adapter. One worker owns the engine; no loader or ambient I/O.
use crate::plugin::{
    api::{event::PluginEventEnvelope, *},
    execution_api::call_json,
};
use rquickjs::{
    function::{Async, Func},
    AsyncContext, AsyncRuntime, Function, Module, Object, Persistent, Promise,
};
use std::{
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
        let config = config.to_string();
        let allowed = serde_json::to_string(&allowed).map_err(|_| PluginError::InvalidInput)?;
        let stopped = self.stopped.clone();
        let pending = Arc::new(tokio::sync::Semaphore::new(16));
        self.set_deadline(Duration::from_secs(10));
        let operation = self.context.async_with(async |ctx| {
            let host = Func::from(Async(move |operation: String, input: String| {
                let api = api.clone(); let stopped = stopped.clone(); let pending = pending.clone();
                async move {
                    let Ok(_permit) = pending.try_acquire() else { return "{\"ok\":false,\"error\":{\"code\":\"limit_exceeded\"}}".to_string(); };
                    tokio::select! { biased;
                        _ = stopped.cancelled() => "{\"ok\":false,\"error\":{\"code\":\"context_expired\"}}".to_string(),
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
            result = tokio::time::timeout(Duration::from_secs(10), operation) => result.map_err(|_| PluginError::Timeout)?.map_err(engine_error),
        };
        self.deliver = Some(result.map_err(|error| self.classify(error))?);
        self.set_deadline(Duration::from_secs(30));
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
    /// @plugin-api
    /// @since 2.0
    /// @permission events.subscribe
    /// @stability stable
    pub async fn deliver(&mut self, envelope: &PluginEventEnvelope) -> PluginResult<()> {
        self.set_deadline(Duration::from_secs(30));
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
            result = tokio::time::timeout(Duration::from_secs(30), operation) => result.map_err(|_| PluginError::Timeout)?.map_err(engine_error),
        };
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
    }
}
fn engine_error(_: rquickjs::Error) -> PluginError {
    PluginError::Unavailable
}

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
