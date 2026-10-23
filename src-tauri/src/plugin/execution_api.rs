//! @plugin-module
//! @layer plugin/adapter
//! @version 2
//! @stability stable
//! Host-bound capability assembly and finite JSON bridge; no internal entity is serialized.
use super::{
    api::{contest::*, http::*, identity::*, log::*, notification::*, storage::*, submission::*, *},
    bounded_json,
    host::manifest::PluginPermission,
    limits::{
        MAX_BRIDGE_INPUT_BYTES, MAX_BRIDGE_OPERATION_BYTES, MAX_BRIDGE_RESPONSE_BYTES,
        PLUGIN_LOGS_PER_MINUTE,
    },
    query::ScopedQueries,
};
use async_trait::async_trait;
use governor::{DefaultDirectRateLimiter, Quota, RateLimiter};
use serde_json::{json, Value};
use std::{num::NonZeroU32, sync::Arc};

/// Host capability factory, injected by the composition root. Managers never construct I/O.
pub trait PluginApiFactory: Send + Sync {
    fn create(
        &self,
        queries: Arc<ScopedQueries>,
        approval: &super::registry::PluginApproval,
        output: Arc<dyn NotificationOutput>,
    ) -> PluginResult<Arc<dyn PluginApi>>;
}

/// @plugin-api
/// @since 2.0
/// @permission notification
/// @stability stable
/// A text-only output port; absent frontend output must fail explicitly.
pub trait NotificationOutput: Send + Sync {
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    fn emit(
        &self,
        plugin_id: &str,
        level: PluginNotificationLevel,
        message: &str,
    ) -> PluginResult<()>;
}
/// @plugin-api
/// @since 2.0
/// @stability stable
/// One authority and rate budget per plugin activation. Storage and HTTP are injected as
/// ports (implemented in infra), so this module never names a concrete I/O type.
pub struct ExecutionApi {
    pub queries: Arc<ScopedQueries>,
    pub storage: Arc<dyn PluginStorageApi>,
    pub http: Arc<dyn PluginHttpApi>,
    output: Arc<dyn NotificationOutput>,
    notifications: DefaultDirectRateLimiter,
    logs: DefaultDirectRateLimiter,
    calls: DefaultDirectRateLimiter,
}
impl ExecutionApi {
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    pub fn new(
        queries: Arc<ScopedQueries>,
        storage: Arc<dyn PluginStorageApi>,
        http: Arc<dyn PluginHttpApi>,
        output: Arc<dyn NotificationOutput>,
    ) -> Self {
        Self {
            queries,
            storage,
            http,
            output,
            notifications: RateLimiter::direct(Quota::per_minute(NonZeroU32::new(10).unwrap())),
            // Log writes never touch a provider; the budget only protects the host log file.
            logs: RateLimiter::direct(Quota::per_minute(
                NonZeroU32::new(PLUGIN_LOGS_PER_MINUTE).unwrap(),
            )),
            // Local capabilities have no provider budget; this only stops runaway loops.
            calls: RateLimiter::direct(Quota::per_minute(NonZeroU32::new(600).unwrap())),
        }
    }
}
#[async_trait]
impl ContextApi for ExecutionApi {
    async fn current_context(&self) -> PluginResult<PluginQueryContext> {
        self.queries.current_context().await
    }
}
#[async_trait]
impl IdentityApi for ExecutionApi {
    async fn current(&self, context: &PluginQueryContext) -> PluginResult<Option<PluginIdentity>> {
        self.queries.identity(context).await
    }
}
#[async_trait]
impl ContestApi for ExecutionApi {
    async fn current(&self, context: &PluginQueryContext) -> PluginResult<Option<PluginContest>> {
        self.queries.contest(context, None).await
    }
    async fn get(
        &self,
        context: &PluginQueryContext,
        id: &str,
    ) -> PluginResult<Option<PluginContest>> {
        self.queries.contest(context, Some(id)).await
    }
}
#[async_trait]
impl SubmissionApi for ExecutionApi {
    async fn get(
        &self,
        context: &PluginQueryContext,
        contest: &str,
        id: &str,
    ) -> PluginResult<Option<PluginSubmission>> {
        self.queries.submission(context, contest, id).await
    }
    async fn list(
        &self,
        context: &PluginQueryContext,
        query: PluginSubmissionQuery,
    ) -> PluginResult<PluginSubmissionPage> {
        self.queries.list(context, query).await
    }
}
#[async_trait]
impl PluginStorageApi for ExecutionApi {
    async fn get(&self, key: &str) -> PluginResult<PluginStorageValue> {
        self.storage.get(key).await
    }
    async fn set(&self, key: &str, value: Value) -> PluginResult<()> {
        self.storage.set(key, value).await
    }
    async fn delete(&self, key: &str) -> PluginResult<()> {
        self.storage.delete(key).await
    }
    async fn compare_and_set(
        &self,
        key: &str,
        expected: PluginStorageValue,
        value: Value,
    ) -> PluginResult<bool> {
        self.storage.compare_and_set(key, expected, value).await
    }
}
#[async_trait]
impl PluginHttpApi for ExecutionApi {
    async fn request(
        &self,
        context: &PluginQueryContext,
        request: PluginHttpRequest,
    ) -> PluginResult<PluginHttpResponse> {
        self.http.request(context, request).await
    }
}
#[async_trait]
impl NotificationApi for ExecutionApi {
    async fn notify(&self, level: PluginNotificationLevel, message: &str) -> PluginResult<()> {
        self.queries.require(PluginPermission::Notification)?;
        validate_message(message)?;
        if self.notifications.check().is_err() {
            return Err(PluginError::LimitExceeded);
        }
        self.output
            .emit(&self.queries.grant.manifest().id, level, message)
    }
}
#[async_trait]
impl LogApi for ExecutionApi {
    async fn write(&self, level: PluginLogLevel, message: &str) -> PluginResult<()> {
        validate_log_message(message)?;
        if self.logs.check().is_err() {
            return Err(PluginError::LimitExceeded);
        }
        let manifest = self.queries.grant.manifest();
        // Newlines/tabs are flattened before the Debug-escaped write so a log line
        // stays greppable even if both defenses are ever changed independently.
        let message = sanitize_log_message(message);
        match level {
            PluginLogLevel::Debug => tracing::debug!(
                plugin_id = ?manifest.id,
                plugin_version = ?manifest.version,
                runtime = "js",
                operation = "plugin.log",
                level = "debug",
                message = ?message
            ),
            PluginLogLevel::Info => tracing::info!(
                plugin_id = ?manifest.id,
                plugin_version = ?manifest.version,
                runtime = "js",
                operation = "plugin.log",
                level = "info",
                message = ?message
            ),
            PluginLogLevel::Warn => tracing::warn!(
                plugin_id = ?manifest.id,
                plugin_version = ?manifest.version,
                runtime = "js",
                operation = "plugin.log",
                level = "warn",
                message = ?message
            ),
            PluginLogLevel::Error => tracing::error!(
                plugin_id = ?manifest.id,
                plugin_version = ?manifest.version,
                runtime = "js",
                operation = "plugin.log",
                level = "error",
                message = ?message
            ),
        }
        Ok(())
    }
}
impl PluginApi for ExecutionApi {
    fn audit_identity(&self) -> Option<(&str, &str)> {
        let manifest = self.queries.grant.manifest();
        Some((&manifest.id, &manifest.version))
    }
    fn admit(&self) -> PluginResult<()> {
        self.calls.check().map_err(|_| PluginError::LimitExceeded)
    }
}

fn decode<T: serde::de::DeserializeOwned>(value: Value) -> PluginResult<T> {
    serde_json::from_value(value).map_err(|_| PluginError::InvalidInput)
}
fn encode<T: serde::Serialize>(value: T) -> PluginResult<Value> {
    // Bound the DTO before duplicating its strings into a serde_json::Value.
    bounded_json::encoded_len(&value, MAX_BRIDGE_RESPONSE_BYTES)?;
    serde_json::to_value(value).map_err(|_| PluginError::Unavailable)
}
async fn dispatch(api: &dyn PluginApi, operation: &str, args: Value) -> PluginResult<Value> {
    let context = || decode::<PluginQueryContext>(args["context"].clone());
    let text = |key: &str| {
        args[key]
            .as_str()
            .map(str::to_owned)
            .ok_or(PluginError::InvalidInput)
    };
    match operation {
        "context.current" => encode(api.current_context().await?),
        "identity.current" => encode(IdentityApi::current(api, &context()?).await?),
        "contest.current" => encode(ContestApi::current(api, &context()?).await?),
        "contest.get" => encode(ContestApi::get(api, &context()?, &text("contestId")?).await?),
        "submission.get" => encode(
            SubmissionApi::get(
                api,
                &context()?,
                &text("contestId")?,
                &text("submissionId")?,
            )
            .await?,
        ),
        "submission.list" => encode(
            api.list(&context()?, decode(args["query"].clone())?)
                .await?,
        ),
        "storage.get" => encode(PluginStorageApi::get(api, &text("key")?).await?),
        "storage.set" => {
            api.set(
                &text("key")?,
                args.get("value")
                    .cloned()
                    .ok_or(PluginError::InvalidInput)?,
            )
            .await?;
            Ok(Value::Null)
        }
        "storage.delete" => {
            api.delete(&text("key")?).await?;
            Ok(Value::Null)
        }
        "storage.compareAndSet" => encode(
            api.compare_and_set(
                &text("key")?,
                decode(args["expected"].clone())?,
                args.get("value")
                    .cloned()
                    .ok_or(PluginError::InvalidInput)?,
            )
            .await?,
        ),
        "http.request" => encode(
            api.request(&context()?, decode(args["request"].clone())?)
                .await?,
        ),
        "notification.notify" => {
            api.notify(decode(args["level"].clone())?, &text("message")?)
                .await?;
            Ok(Value::Null)
        }
        "log.write" => {
            LogApi::write(api, decode(args["level"].clone())?, &text("message")?).await?;
            Ok(Value::Null)
        }
        _ => Err(PluginError::Unsupported),
    }
}
/// @plugin-api
/// @since 2.0
/// @permission checked per operation
/// @stability stable
/// Serialized stable success/error result. No Rust error text or stack crosses the bridge.
pub async fn call_json(api: &dyn PluginApi, operation: String, input: String) -> String {
    let result =
        if input.len() > MAX_BRIDGE_INPUT_BYTES || operation.len() > MAX_BRIDGE_OPERATION_BYTES {
            Err(PluginError::LimitExceeded)
        } else if let Err(error) = api.admit() {
            Err(error)
        } else {
            match serde_json::from_str(&input) {
                Ok(args) => dispatch(api, &operation, args).await,
                Err(_) => Err(PluginError::InvalidInput),
            }
        };
    let (response, success) = encode_response(result);
    if let Some((plugin_id, plugin_version)) = api.audit_identity() {
        let logged_operation = operation.chars().take(64).collect::<String>();
        let permission = match operation.split('.').next().unwrap_or("") {
            "submission" => "submission.read",
            "contest" => "contest.read",
            "identity" => "identity.read",
            "storage" => {
                if operation == "storage.get" {
                    "storage.read"
                } else if operation == "storage.compareAndSet" {
                    "storage.read storage.write"
                } else {
                    "storage.write"
                }
            }
            "http" => "network.request",
            "notification" => "notification",
            "log" => "none",
            _ => "none",
        };
        tracing::debug!(plugin_id=?plugin_id, plugin_version=?plugin_version, runtime="js",event="",operation=?logged_operation,permission,
            request_id=%uuid::Uuid::new_v4(),success,"Plugin API completed");
    }
    response
}

fn encode_response(result: PluginResult<Value>) -> (String, bool) {
    let success = result.is_ok();
    let value = match result {
        Ok(value) => json!({"ok":true,"value":value}),
        Err(error) => json!({"ok":false,"error":error}),
    };
    match bounded_json::to_string(&value, MAX_BRIDGE_RESPONSE_BYTES) {
        Ok(response) => (response, success),
        Err(error) => (json!({"ok":false,"error":error}).to_string(), false),
    }
}

#[cfg(test)]
#[path = "tests/execution_api_tests.rs"]
mod tests;
