//! @plugin-module
//! @layer plugin/adapter
//! @version 2
//! @stability stable
//! Permission checked read projections and bounded, opaque, scope-bound cursors.
use super::{
    api::{contest::PluginContest, identity::PluginIdentity, submission::*, *},
    bounded_json,
    host::manifest::PluginPermission,
    limits::{
        MAX_CURSORS, MAX_CURSOR_BUFFER_BYTES, MAX_CURSOR_PAGE_BYTES, MAX_CURSOR_RESPONSE_BYTES,
        MAX_QUERY_CONTEXTS, PLUGIN_QUERIES_PER_MINUTE, PLUGIN_QUERY_CONCURRENCY, PROVIDER_TIMEOUT,
        QUERY_HANDLE_TTL, SHARED_QUERIES_PER_MINUTE, SHARED_QUERY_CONCURRENCY,
        SUBMISSION_LOOKUP_TIMEOUT,
    },
    permission::PluginGrant,
};
use crate::{
    core::{
        access::{AccessRevision, AccessSnapshot},
        entity::{contest::Contest, submission::SubmissionHistoryPage},
        error::AppError,
        provider::registry::ProviderRegistry,
        repository::config_repo::ConfigRepository,
    },
    service::{
        auth::AuthService, config::ConfigService, contest::ContestService,
        submission::SubmissionService,
    },
};
use async_trait::async_trait;
use governor::{DefaultDirectRateLimiter, Quota, RateLimiter};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    num::NonZeroU32,
    sync::{Arc, Mutex},
    time::Instant,
};
use tokio_util::sync::CancellationToken;

/// Host-private OJ request budget. Counts each actual provider dispatch, including
/// every native page scanned by submission.get; exhausted callers never queue.
pub(crate) struct QueryBudget {
    rate: DefaultDirectRateLimiter,
    concurrent: tokio::sync::Semaphore,
}
impl QueryBudget {
    pub(crate) fn new(per_minute: u32, concurrent: usize) -> Self {
        Self {
            rate: RateLimiter::direct(Quota::per_minute(NonZeroU32::new(per_minute).unwrap())),
            concurrent: tokio::sync::Semaphore::new(concurrent),
        }
    }
    /// Admit one provider dispatch against the activation budget and the shared budget.
    /// Both concurrency permits are taken before any rate token: permits are refunded on
    /// drop while tokens are not, so a rejection by either concurrency limit never costs
    /// a token. Tokens are spent activation-first; if only the shared bucket is empty the
    /// activation has spent one of its own tokens, which cannot drain other plugins.
    fn acquire_pair<'a>(
        local: &'a Self,
        shared: &'a Self,
    ) -> PluginResult<(
        tokio::sync::SemaphorePermit<'a>,
        tokio::sync::SemaphorePermit<'a>,
    )> {
        let local_permit = local
            .concurrent
            .try_acquire()
            .map_err(|_| PluginError::LimitExceeded)?;
        let shared_permit = shared
            .concurrent
            .try_acquire()
            .map_err(|_| PluginError::LimitExceeded)?;
        local.rate.check().map_err(|_| PluginError::LimitExceeded)?;
        shared
            .rate
            .check()
            .map_err(|_| PluginError::LimitExceeded)?;
        Ok((local_permit, shared_permit))
    }
}

/// @plugin-api
/// @since 2.0
/// @stability stable
/// Host-private scope metadata; never serialized across the bridge.
#[derive(Clone, PartialEq, Eq)]
pub struct QueryScope {
    pub oj: String,
    pub endpoint: String,
    pub user: Option<(String, String)>,
    pub contest: Option<String>,
}

/// @plugin-api
/// @since 2.0
/// @stability stable
/// Read-only service port; tests can provide a deterministic OJ without live services.
#[async_trait]
pub trait QuerySource: Send + Sync {
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    /// Snapshot current effective access without restoring credentials. `plugin_id` is
    /// diagnostic attribution only: it never changes the resolved scope.
    fn scope(&self, plugin_id: &str) -> PluginResult<QueryScope>;
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    /// Cheap cache probe for the async fast path. Implementations must not perform I/O;
    /// the default `None` keeps the blocking read for every call.
    fn cached_scope(&self) -> Option<QueryScope> {
        None
    }
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    /// Read metadata without selecting a contest.
    async fn contest(&self, id: &str) -> PluginResult<Contest>;
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    /// Read one native page of own records with honest continuation evidence.
    async fn history(&self, contest: &str, page: u32) -> PluginResult<SubmissionHistoryPage>;
}

/// Read port for the configured OJ instances. Keeps this adapter independent of which
/// repository backs the configuration; production wires `ConfigService`.
pub trait OjInstanceConfig: Send + Sync {
    /// Configured base URL of the enabled instance with this id (`None` when the instance
    /// is absent or disabled).
    fn enabled_base_url(&self, oj: &str) -> Option<String>;
}
impl<R: ConfigRepository> OjInstanceConfig for ConfigService<R> {
    fn enabled_base_url(&self, oj: &str) -> Option<String> {
        self.get()
            .oj
            .instances
            .into_iter()
            .find(|i| i.enabled && i.id == oj)
            .map(|i| i.base_url)
    }
}

/// @plugin-api
/// @since 2.0
/// @stability stable
/// Production service wrapper; no commands or source-code queries.
pub struct ServiceQueries {
    pub auth: Arc<AuthService>,
    pub contest: Arc<ContestService>,
    pub submission: Arc<SubmissionService>,
    pub registry: Arc<dyn ProviderRegistry>,
    pub config: Arc<dyn OjInstanceConfig>,
    pub access: Arc<AccessRevision>,
    pub cached_scope: Mutex<Option<(AccessSnapshot, QueryScope)>>,
    /// (revision, plugin_id) pairs that already emitted a scope-resolution warning. A
    /// disabled or unconfigured OJ is persistent state; warning once per plugin per
    /// revision keeps attribution accurate and stops any single plugin flooding the log
    /// on every bridge call.
    pub(crate) scope_warnings: Mutex<ScopeWarnings>,
}
/// Per-revision set of plugin ids that already warned; cleared when the revision changes.
#[derive(Default)]
pub(crate) struct ScopeWarnings {
    revision: Option<u64>,
    seen: HashSet<String>,
}
impl ServiceQueries {
    /// Warn once per access revision; a disabled or unconfigured OJ is persistent state,
    /// not a per-call event, and the log line carries host-owned attribution only.
    fn warn_scope_failure(
        &self,
        revision: u64,
        plugin_id: &str,
        oj: &str,
        category: &'static str,
        message: &'static str,
    ) {
        {
            let mut warnings = self
                .scope_warnings
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if warnings.revision != Some(revision) {
                warnings.revision = Some(revision);
                warnings.seen.clear();
            }
            // First failure for this plugin at this revision warns; repeats are suppressed
            // so the single line carries accurate per-plugin attribution.
            if !warnings.seen.insert(plugin_id.to_string()) {
                return;
            }
        }
        tracing::warn!(
            plugin_id = ?plugin_id,
            oj_id = ?oj,
            runtime = "js",
            operation = "scope.resolve",
            category,
            message,
            "Plugin scope resolution failed"
        );
    }
    /// Record the failure class before the plugin-visible error is reduced to a stable
    /// code. Provider messages may embed URLs, credentials or response bodies, so only
    /// the variant and host-owned identifiers are logged.
    fn log_provider_failure(&self, operation: &'static str, error: &AppError) {
        tracing::warn!(
            oj_id = ?self.registry.current_id(),
            runtime = "js",
            operation,
            category = provider_error_category(error),
            "Plugin OJ query failed"
        );
    }
}

/// Stable classification of a provider failure; never derived from the error message.
fn provider_error_category(error: &AppError) -> &'static str {
    match error {
        AppError::Auth(_) => "auth",
        AppError::Contest(_) => "contest",
        AppError::Problem(_) => "problem",
        AppError::Submission(_) => "submission",
        AppError::Workspace(_) => "workspace",
        AppError::Io(_) => "io",
        AppError::Network(_) => "network",
        AppError::Config(_) => "config",
        AppError::ProviderNotFound(_) => "provider_not_found",
        AppError::Serialization(_) => "serialization",
        AppError::Unknown(_) => "unknown",
    }
}
/// Single source of truth for "does this cached scope still match the live snapshot".
/// Both the fast path (`cached_scope`) and the slow path (`scope`) must agree, or a token
/// revoked by an OJ/account/contest switch could be served from cache. Keeping it in one
/// function makes the two paths structurally incapable of drifting apart.
fn scope_cache_hit(cached: &AccessSnapshot, snapshot: &AccessSnapshot) -> bool {
    cached.revision == snapshot.revision
        && !cached.cancellation.is_cancelled()
        && !snapshot.cancellation.is_cancelled()
}
#[async_trait]
impl QuerySource for ServiceQueries {
    fn cached_scope(&self) -> Option<QueryScope> {
        // Fast path: no session or config I/O, just the shared hit predicate.
        let snapshot = self.access.snapshot()?;
        let cache = self.cached_scope.lock().unwrap_or_else(|e| e.into_inner());
        cache
            .as_ref()
            .filter(|(cached, _)| scope_cache_hit(cached, &snapshot))
            .map(|(_, scope)| scope.clone())
    }
    fn scope(&self, plugin_id: &str) -> PluginResult<QueryScope> {
        let snapshot = self.access.snapshot().ok_or(PluginError::ContextExpired)?;
        // Only the clone happens under the lock: session/config reads below are I/O and
        // must not serialize every plugin's scope check.
        {
            let cache = self.cached_scope.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((cached, scope)) = cache.as_ref() {
                if scope_cache_hit(cached, &snapshot) {
                    return Ok(scope.clone());
                }
            }
        }
        let oj = self.registry.current_id().to_string();
        let endpoint = self.registry.current_endpoint().ok_or_else(|| {
            self.warn_scope_failure(
                snapshot.revision,
                plugin_id,
                &oj,
                "endpoint_missing",
                "Running OJ provider has no endpoint metadata",
            );
            PluginError::Unavailable
        })?;
        let Some(configured_url) = self.config.enabled_base_url(&oj) else {
            self.warn_scope_failure(
                snapshot.revision,
                plugin_id,
                &oj,
                "instance_disabled_or_missing",
                "Running OJ is no longer enabled in configuration",
            );
            return Err(PluginError::Unavailable);
        };
        // Existing instance URLs intentionally take effect after restart (SettingsView).
        // Bind queries and identity keys to the running provider, never to a pending URL.
        let pending_restart = configured_url != endpoint;
        let scope = QueryScope {
            oj,
            endpoint,
            user: self.auth.identity_metadata().map_err(|error| {
                self.log_provider_failure("scope.identity", &error);
                PluginError::Unavailable
            })?,
            contest: self.contest.current_contest_id(),
        };
        if snapshot.cancellation.is_cancelled() {
            return Err(PluginError::ContextExpired);
        }
        *self.cached_scope.lock().unwrap_or_else(|e| e.into_inner()) =
            Some((snapshot, scope.clone()));
        if pending_restart {
            // Successful snapshots are cached per revision; no URL/credentials enter the log.
            tracing::warn!(runtime="js", operation="scope.resolve", category="endpoint_pending_restart", "OJ endpoint change is pending restart; plugin queries keep using the running provider");
        }
        Ok(scope)
    }
    async fn contest(&self, id: &str) -> PluginResult<Contest> {
        self.contest.read_metadata(id).await.map_err(|error| {
            self.log_provider_failure("contest.get", &error);
            PluginError::ProviderError
        })
    }
    async fn history(&self, contest: &str, page: u32) -> PluginResult<SubmissionHistoryPage> {
        self.submission
            .read_own_history(contest, page)
            .await
            .map_err(|error| {
                self.log_provider_failure("submission.history", &error);
                PluginError::ProviderError
            })
    }
}

#[derive(Clone)]
struct Context {
    scope: QueryScope,
    revision: u64,
    created: Instant,
    cancellation: CancellationToken,
}
struct Cursor {
    context: String,
    query: PluginSubmissionQuery,
    native_page: u32,
    pending: VecDeque<PluginSubmission>,
    more: bool,
    total: Option<u64>,
    created: Instant,
    buffered_bytes: usize,
}
#[derive(Default)]
struct State {
    contexts: HashMap<String, Context>,
    cursors: HashMap<String, Cursor>,
}

/// Removes an unpublished/internal continuation even when the owning future is dropped.
struct CursorLease<'a> {
    state: &'a Mutex<State>,
    id: Option<String>,
}
impl Drop for CursorLease<'_> {
    fn drop(&mut self) {
        if let Some(id) = &self.id {
            self.state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .cursors
                .remove(id);
        }
    }
}

/// @plugin-api
/// @since 2.0
/// @stability stable
/// Per-activation authority. Stop revokes every handle; old scopes cannot survive ABA changes.
pub struct ScopedQueries {
    pub grant: PluginGrant,
    pub stopped: CancellationToken,
    source: Arc<dyn QuerySource>,
    access: Arc<AccessRevision>,
    state: Mutex<State>,
    budget: QueryBudget,
    shared_budget: Arc<QueryBudget>,
}
impl ScopedQueries {
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    pub fn new(
        grant: PluginGrant,
        source: Arc<dyn QuerySource>,
        access: Arc<AccessRevision>,
        stopped: CancellationToken,
    ) -> Self {
        Self {
            grant,
            source,
            access,
            stopped,
            state: Mutex::default(),
            budget: QueryBudget::new(PLUGIN_QUERIES_PER_MINUTE, PLUGIN_QUERY_CONCURRENCY),
            shared_budget: Arc::new(QueryBudget::new(
                SHARED_QUERIES_PER_MINUTE,
                SHARED_QUERY_CONCURRENCY,
            )),
        }
    }
    pub(crate) fn with_shared_budget(mut self, budget: Arc<QueryBudget>) -> Self {
        self.shared_budget = budget;
        self
    }
    async fn scope(&self) -> PluginResult<QueryScope> {
        // Cache hits need no I/O: avoid a blocking-pool round trip on the hot path where
        // every bridge call checks the scope two or three times.
        if let Some(scope) = self.source.cached_scope() {
            return Ok(scope);
        }
        // identity_metadata reads the session repository. Never execute that I/O
        // on the Tokio executor, including checks before/after remote requests.
        let source = self.source.clone();
        let plugin_id = self.grant.manifest().id.clone();
        let dispatch = tracing::dispatcher::get_default(Clone::clone);
        tokio::task::spawn_blocking(move || {
            tracing::dispatcher::with_default(&dispatch, || source.scope(&plugin_id))
        })
        .await
        .map_err(|_| PluginError::Unavailable)?
    }
    async fn remote<T>(
        &self,
        handle: &PluginQueryContext,
        operation: impl std::future::Future<Output = PluginResult<T>>,
    ) -> PluginResult<T> {
        let cancellation = self.check(handle).await?;
        let _permits = QueryBudget::acquire_pair(&self.budget, &self.shared_budget)?;
        let result = tokio::select! { biased;
            _ = self.stopped.cancelled() => return Err(PluginError::ContextExpired),
            _ = cancellation.cancelled() => return Err(PluginError::ContextExpired),
            result = tokio::time::timeout(PROVIDER_TIMEOUT, operation) => {
                result.unwrap_or(Err(PluginError::Timeout))
            },
        };
        self.check(handle).await?;
        result
    }
    /// @plugin-api
    /// @since 2.0
    /// @stability stable
    pub fn require(&self, permission: PluginPermission) -> PluginResult<()> {
        if self.stopped.is_cancelled() {
            return Err(PluginError::ContextExpired);
        }
        self.grant.require(permission)
    }
    /// @plugin-api
    /// @since 2.0
    /// @stability stable
    /// Checks scope before dispatch and returns its cancellation fence.
    pub async fn check(&self, handle: &PluginQueryContext) -> PluginResult<CancellationToken> {
        if self.stopped.is_cancelled() {
            return Err(PluginError::ContextExpired);
        }
        let context = self
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contexts
            .get(handle.host_id())
            .cloned()
            .ok_or(PluginError::ContextExpired)?;
        let snapshot = self.access.snapshot().ok_or(PluginError::ContextExpired)?;
        if context.created.elapsed() > QUERY_HANDLE_TTL
            || snapshot.revision != context.revision
            || self.scope().await.map_err(|_| PluginError::ContextExpired)? != context.scope
            // Check after scope(): a transition may revoke the token during the snapshot.
            || context.cancellation.is_cancelled()
            || self.stopped.is_cancelled()
        {
            return Err(PluginError::ContextExpired);
        }
        Ok(context.cancellation)
    }
    async fn context(&self, handle: &PluginQueryContext) -> PluginResult<Context> {
        self.check(handle).await?;
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contexts
            .get(handle.host_id())
            .cloned()
            .ok_or(PluginError::ContextExpired)
    }
    /// @plugin-api
    /// @since 2.0
    /// @permission none
    /// @stability stable
    pub async fn current_context(&self) -> PluginResult<PluginQueryContext> {
        if self.stopped.is_cancelled() {
            return Err(PluginError::ContextExpired);
        }
        let snapshot = self.access.snapshot().ok_or(PluginError::ContextExpired)?;
        let scope = self.scope().await?;
        if snapshot.cancellation.is_cancelled() || self.stopped.is_cancelled() {
            return Err(PluginError::ContextExpired);
        }
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.contexts.retain(|_, c| {
            c.created.elapsed() < QUERY_HANDLE_TTL && !c.cancellation.is_cancelled()
        });
        if state.contexts.len() >= MAX_QUERY_CONTEXTS {
            return Err(PluginError::LimitExceeded);
        }
        let id = uuid::Uuid::new_v4().to_string();
        state.contexts.insert(
            id.clone(),
            Context {
                scope,
                revision: snapshot.revision,
                cancellation: snapshot.cancellation,
                created: Instant::now(),
            },
        );
        Ok(PluginQueryContext::from_host_id(id))
    }
    /// @plugin-api
    /// @since 2.0
    /// @permission identity.read
    /// @stability stable
    pub async fn identity(
        &self,
        handle: &PluginQueryContext,
    ) -> PluginResult<Option<PluginIdentity>> {
        self.require(PluginPermission::IdentityRead)?;
        let context = self.context(handle).await?;
        Ok(context.scope.user.map(|(id, _)| {
            let key = serde_json::to_vec(&(&context.scope.oj, &context.scope.endpoint, id))
                .expect("identity key contains only JSON-serializable strings");
            PluginIdentity {
                oj_id: context.scope.oj,
                user_key: format!("{:x}", Sha256::digest(key)),
            }
        }))
    }
    /// @plugin-api
    /// @since 2.0
    /// @permission contest.read
    /// @stability stable
    pub async fn contest(
        &self,
        handle: &PluginQueryContext,
        id: Option<&str>,
    ) -> PluginResult<Option<PluginContest>> {
        self.require(PluginPermission::ContestRead)?;
        let context = self.context(handle).await?;
        let Some(contest) = context.scope.contest else {
            return Ok(None);
        };
        if id.is_some_and(|id| id != contest) {
            return Ok(None);
        }
        let result = self.remote(handle, self.source.contest(&contest)).await;
        let result = result.and_then(|c| {
            if c.id == contest {
                Ok(c)
            } else {
                Err(PluginError::ProviderError)
            }
        });
        result.map(|c| {
            Some(PluginContest {
                contest_id: c.id,
                title: c.title.chars().take(256).collect(),
                starts_at: c.start_time.saturating_mul(1000),
                ends_at: c.end_time.saturating_mul(1000),
            })
        })
    }
    /// Fetch and project one native page when the cursor has nothing buffered. On any
    /// failure the cursor is left untouched, so the caller's restore puts back a true
    /// pre-call snapshot; this never mutates the shared cursor table.
    async fn advance(
        &self,
        handle: &PluginQueryContext,
        query: &PluginSubmissionQuery,
        owner: &str,
        cursor: &mut Cursor,
    ) -> PluginResult<()> {
        if !cursor.pending.is_empty() || !cursor.more {
            return Ok(());
        }
        if cursor.native_page > 10_000 {
            return Err(PluginError::LimitExceeded);
        }
        let page = self
            .remote(
                handle,
                self.source.history(&query.contest_id, cursor.native_page),
            )
            .await?;
        if page.records.len() > 1000
            || page
                .records
                .iter()
                .any(|r| r.owner_id.as_deref() != Some(owner))
        {
            return Err(PluginError::ProviderError);
        }
        let has_more = page.has_more;
        let total = if query.problem_id.is_none() {
            page.total
        } else {
            None
        };
        let pending: VecDeque<PluginSubmission> = page
            .records
            .into_iter()
            .filter(|r| query.problem_id.as_ref().is_none_or(|id| *id == r.pid))
            .map(|r| PluginSubmission {
                submission_id: r.submit_id,
                contest_id: query.contest_id.clone(),
                problem_id: r.pid,
                problem_display_id: r.display_id,
                status: r.status.as_str().into(),
                submitted_at: r.submit_time.saturating_mul(1000),
            })
            .collect();
        // Validate the encoded size before committing any mutation. A failed fetch must
        // leave the caller's cursor at its true pre-call snapshot, so the restore path in
        // `list` can never retain an oversized page with stale `buffered_bytes` (which
        // would silently bypass the per-cursor and aggregate memory budgets).
        bounded_json::encoded_len(&pending, MAX_CURSOR_PAGE_BYTES)?;
        cursor.native_page += 1;
        cursor.more = has_more;
        cursor.total = total;
        cursor.pending = pending;
        Ok(())
    }
    /// @plugin-api
    /// @since 2.0
    /// @permission submission.read
    /// @stability stable
    /// Opaque cursors retain native page remainders. An empty filtered page may still have a cursor.
    pub async fn list(
        &self,
        handle: &PluginQueryContext,
        query: PluginSubmissionQuery,
    ) -> PluginResult<PluginSubmissionPage> {
        self.require(PluginPermission::SubmissionRead)?;
        query.validate()?;
        let context = self.context(handle).await?;
        let (owner, _) = context.scope.user.ok_or(PluginError::Unauthenticated)?;
        if context.scope.contest.as_deref() != Some(&query.contest_id) {
            return Err(PluginError::ContextExpired);
        }
        let mut cursor = if let Some(id) = &query.cursor {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            let c = state.cursors.get(id).ok_or(PluginError::ContextExpired)?;
            if c.context != handle.host_id()
                || c.query.contest_id != query.contest_id
                || c.query.problem_id != query.problem_id
                || c.query.limit != query.limit
                || c.created.elapsed() > QUERY_HANDLE_TTL
            {
                return Err(PluginError::ContextExpired);
            }
            // Consume under the same lock as validation: concurrent reuse cannot clone it twice.
            state
                .cursors
                .remove(id)
                .ok_or(PluginError::ContextExpired)?
        } else {
            Cursor {
                context: handle.host_id().into(),
                query: query.clone(),
                native_page: 1,
                pending: VecDeque::new(),
                more: true,
                total: None,
                created: Instant::now(),
                buffered_bytes: 0,
            }
        };
        if let Err(error) = self
            .advance(handle, &query, owner.as_str(), &mut cursor)
            .await
        {
            // A retryable failure (budget, provider timeout, transient IO) must not consume
            // the caller's cursor: restore the pre-call snapshot so a retry resumes here
            // instead of restarting from page one. An expired context stays revoked.
            if error != PluginError::ContextExpired {
                if let Some(id) = query.cursor.clone() {
                    self.state
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .cursors
                        .insert(id, cursor);
                }
            }
            return Err(error);
        }
        let records = cursor
            .pending
            .drain(..cursor.pending.len().min(query.limit as usize))
            .collect();
        let total = cursor.total;
        let next = if cursor.more || !cursor.pending.is_empty() {
            cursor.buffered_bytes =
                bounded_json::encoded_len(&cursor.pending, MAX_CURSOR_PAGE_BYTES)?;
            Some(uuid::Uuid::new_v4().to_string())
        } else {
            None
        };
        // Bound the whole response, including the bridge envelope, before publishing the
        // continuation: a page that cannot be encoded must not leave a cursor the plugin
        // never received.
        let page = PluginSubmissionPage {
            records,
            total,
            next_cursor: next.clone(),
        };
        bounded_json::encoded_len(&page, MAX_CURSOR_RESPONSE_BYTES)?;
        if let Some(id) = &next {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            state
                .cursors
                .retain(|_, c| c.created.elapsed() < QUERY_HANDLE_TTL);
            let retained = state
                .cursors
                .values()
                .map(|c| c.buffered_bytes)
                .sum::<usize>();
            if state.cursors.len() >= MAX_CURSORS
                || retained + cursor.buffered_bytes > MAX_CURSOR_BUFFER_BYTES
            {
                return Err(PluginError::LimitExceeded);
            }
            state.cursors.insert(id.clone(), cursor);
        }
        let mut continuation = CursorLease {
            state: &self.state,
            id: next,
        };
        self.check(handle).await?;
        Ok(PluginSubmissionPage {
            records: page.records,
            total: page.total,
            next_cursor: continuation.id.take(),
        })
    }
    /// @plugin-api
    /// @since 2.0
    /// @permission submission.read
    /// @stability stable
    /// Metadata lookup scans own contest history; hitting the scan budget is an error, never false absence.
    pub async fn submission(
        &self,
        handle: &PluginQueryContext,
        contest: &str,
        id: &str,
    ) -> PluginResult<Option<PluginSubmission>> {
        if id.is_empty() || id.len() > 256 {
            return Err(PluginError::InvalidInput);
        }
        let mut continuation = CursorLease {
            state: &self.state,
            id: None,
        };
        let scan = async {
            for _ in 0..100 {
                let page = self
                    .list(
                        handle,
                        PluginSubmissionQuery {
                            contest_id: contest.into(),
                            problem_id: None,
                            cursor: continuation.id.clone(),
                            limit: 100,
                        },
                    )
                    .await?;
                continuation.id = page.next_cursor;
                if let Some(record) = page.records.into_iter().find(|r| r.submission_id == id) {
                    return Ok(Some(record));
                }
                if continuation.id.is_none() {
                    return Ok(None);
                }
            }
            Err(PluginError::LimitExceeded)
        };
        // A whole lookup, including scope checks and all native pages, must finish
        // before the shortest JS entry deadline so timeout remains catchable in JS.
        tokio::time::timeout(SUBMISSION_LOOKUP_TIMEOUT, scan)
            .await
            .unwrap_or(Err(PluginError::Timeout))
    }
}

#[cfg(test)]
#[path = "tests/query_tests.rs"]
mod tests;
