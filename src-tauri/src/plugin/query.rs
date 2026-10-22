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
        MAX_CURSOR_BUFFER_BYTES, MAX_CURSOR_PAGE_BYTES, MAX_CURSOR_RESPONSE_BYTES,
        PROVIDER_TIMEOUT, SUBMISSION_LOOKUP_TIMEOUT,
    },
    permission::PluginGrant,
};
use crate::{
    core::{
        access::{AccessRevision, AccessSnapshot},
        entity::{contest::Contest, submission::SubmissionHistoryPage},
        provider::registry::ProviderRegistry,
    },
    infra::fs_config_repo::FsConfigRepository,
    service::{
        auth::AuthService, config::ConfigService, contest::ContestService,
        submission::SubmissionService,
    },
};
use async_trait::async_trait;
use governor::{DefaultDirectRateLimiter, Quota, RateLimiter};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, VecDeque},
    num::NonZeroU32,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
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
    fn acquire(&self) -> PluginResult<tokio::sync::SemaphorePermit<'_>> {
        let permit = self
            .concurrent
            .try_acquire()
            .map_err(|_| PluginError::LimitExceeded)?;
        self.rate.check().map_err(|_| PluginError::LimitExceeded)?;
        Ok(permit)
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
    /// Read metadata without selecting a contest.
    async fn contest(&self, id: &str) -> PluginResult<Contest>;
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    /// Read one native page of own records with honest continuation evidence.
    async fn history(&self, contest: &str, page: u32) -> PluginResult<SubmissionHistoryPage>;
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
    pub config: Arc<ConfigService<FsConfigRepository>>,
    pub access: Arc<AccessRevision>,
    pub cached_scope: Mutex<Option<(AccessSnapshot, QueryScope)>>,
    /// Last revision that emitted a scope-resolution warning; persistent misconfiguration
    /// must not let a plugin flood the log on every bridge call.
    pub(crate) last_scope_warning: Mutex<Option<u64>>,
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
        let mut last = self
            .last_scope_warning
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if *last == Some(revision) {
            return;
        }
        *last = Some(revision);
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
}
#[async_trait]
impl QuerySource for ServiceQueries {
    fn scope(&self, plugin_id: &str) -> PluginResult<QueryScope> {
        let snapshot = self.access.snapshot().ok_or(PluginError::ContextExpired)?;
        // Only the clone happens under the lock: session/config reads below are I/O and
        // must not serialize every plugin's scope check.
        {
            let cache = self.cached_scope.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((cached, scope)) = cache.as_ref() {
                if cached.revision == snapshot.revision
                    && !cached.cancellation.is_cancelled()
                    && !snapshot.cancellation.is_cancelled()
                {
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
        let configured = self.config.get();
        let Some(instance) = configured
            .oj
            .instances
            .iter()
            .find(|i| i.enabled && i.id == oj)
        else {
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
        let pending_restart = instance.base_url != endpoint;
        let scope = QueryScope {
            oj,
            endpoint,
            user: self
                .auth
                .identity_metadata()
                .map_err(|_| PluginError::Unavailable)?,
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
        self.contest
            .read_metadata(id)
            .await
            .map_err(|_| PluginError::ProviderError)
    }
    async fn history(&self, contest: &str, page: u32) -> PluginResult<SubmissionHistoryPage> {
        self.submission
            .read_own_history(contest, page)
            .await
            .map_err(|_| PluginError::ProviderError)
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
            budget: QueryBudget::new(60, 2),
            shared_budget: Arc::new(QueryBudget::new(120, 4)),
        }
    }
    pub(crate) fn with_shared_budget(mut self, budget: Arc<QueryBudget>) -> Self {
        self.shared_budget = budget;
        self
    }
    async fn scope(&self) -> PluginResult<QueryScope> {
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
        let _local = self.budget.acquire()?;
        let _shared = self.shared_budget.acquire()?;
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
        if context.created.elapsed() > Duration::from_secs(300)
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
            c.created.elapsed() < Duration::from_secs(300) && !c.cancellation.is_cancelled()
        });
        if state.contexts.len() >= 32 {
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
                || c.created.elapsed() > Duration::from_secs(300)
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
        if cursor.pending.is_empty() && cursor.more {
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
                    .any(|r| r.owner_id.as_deref() != Some(owner.as_str()))
            {
                return Err(PluginError::ProviderError);
            }
            cursor.native_page += 1;
            cursor.more = page.has_more;
            cursor.total = if query.problem_id.is_none() {
                page.total
            } else {
                None
            };
            cursor.pending = page
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
            bounded_json::encoded_len(&cursor.pending, MAX_CURSOR_PAGE_BYTES)?;
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
                .retain(|_, c| c.created.elapsed() < Duration::from_secs(300));
            let retained = state
                .cursors
                .values()
                .map(|c| c.buffered_bytes)
                .sum::<usize>();
            if state.cursors.len() >= 32
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
