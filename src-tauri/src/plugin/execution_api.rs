//! @plugin-module
//! @layer plugin/adapter
//! @version 2
//! @stability stable
//! Host-bound capability assembly and finite JSON bridge; no internal entity is serialized.
use super::{
    api::{contest::*, http::*, identity::*, notification::*, storage::*, submission::*, *},
    host::manifest::PluginPermission,
    query::ScopedQueries,
};
use crate::infra::{plugin_http::PluginHttp, plugin_storage::JsonPluginStorage};
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
/// One authority and rate budget per plugin activation.
pub struct ExecutionApi {
    pub queries: Arc<ScopedQueries>,
    pub storage: JsonPluginStorage,
    pub http: PluginHttp,
    output: Arc<dyn NotificationOutput>,
    notifications: DefaultDirectRateLimiter,
}
impl ExecutionApi {
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    pub fn new(
        queries: Arc<ScopedQueries>,
        storage: JsonPluginStorage,
        http: PluginHttp,
        output: Arc<dyn NotificationOutput>,
    ) -> Self {
        Self {
            queries,
            storage,
            http,
            output,
            notifications: RateLimiter::direct(Quota::per_minute(NonZeroU32::new(10).unwrap())),
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
impl PluginApi for ExecutionApi {
    fn audit_identity(&self) -> Option<(&str, &str)> {
        let manifest = self.queries.grant.manifest();
        Some((&manifest.id, &manifest.version))
    }
}

fn decode<T: serde::de::DeserializeOwned>(value: Value) -> PluginResult<T> {
    serde_json::from_value(value).map_err(|_| PluginError::InvalidInput)
}
fn encode<T: serde::Serialize>(value: T) -> PluginResult<Value> {
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
        _ => Err(PluginError::Unsupported),
    }
}
/// @plugin-api
/// @since 2.0
/// @permission checked per operation
/// @stability stable
/// Serialized stable success/error result. No Rust error text or stack crosses the bridge.
pub async fn call_json(api: &dyn PluginApi, operation: String, input: String) -> String {
    let result = if input.len() > 140_000 || operation.len() > 64 {
        Err(PluginError::LimitExceeded)
    } else {
        match serde_json::from_str(&input) {
            Ok(args) => dispatch(api, &operation, args).await,
            Err(_) => Err(PluginError::InvalidInput),
        }
    };
    if let Some((plugin_id, plugin_version)) = api.audit_identity() {
        let logged_operation = operation.chars().take(64).collect::<String>();
        let permission = match operation.split('.').next().unwrap_or("") {
            "submission" => "submission.read",
            "contest" => "contest.read",
            "identity" => "identity.read",
            "storage" => {
                if operation == "storage.get" {
                    "storage.read"
                } else {
                    "storage.write"
                }
            }
            "http" => "network.request",
            "notification" => "notification",
            _ => "none",
        };
        tracing::debug!(plugin_id=?plugin_id, plugin_version=?plugin_version, runtime="js",event="",operation=?logged_operation,permission,
            request_id=%uuid::Uuid::new_v4(),success=result.is_ok(),"Plugin API completed");
    }
    match result {
        Ok(value) => json!({"ok":true,"value":value}),
        Err(error) => json!({"ok":false,"error":error}),
    }
    .to_string()
}
