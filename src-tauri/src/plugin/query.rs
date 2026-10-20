//! @plugin-module
//! @layer plugin/adapter
//! @version 2
//! @stability stable
//! Permission checked read projections and bounded, opaque, scope-bound cursors.
use super::{
    api::{contest::PluginContest, identity::PluginIdentity, submission::*, *},
    host::manifest::PluginPermission,
    permission::PluginGrant,
};
use crate::{
    core::{
        access::AccessRevision,
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
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

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
    /// Snapshot current effective access without restoring credentials.
    fn scope(&self) -> PluginResult<QueryScope>;
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
}
#[async_trait]
impl QuerySource for ServiceQueries {
    fn scope(&self) -> PluginResult<QueryScope> {
        let oj = self.registry.current_id().to_string();
        let endpoint = self
            .registry
            .current_endpoint()
            .ok_or(PluginError::Unavailable)?;
        let configured = self.config.get();
        // Editing a URL does not rebuild an existing provider. Never label an old provider with a new URL.
        if !configured
            .oj
            .instances
            .iter()
            .any(|i| i.enabled && i.id == oj && i.base_url == endpoint)
        {
            return Err(PluginError::Unavailable);
        }
        Ok(QueryScope {
            oj,
            endpoint,
            user: self
                .auth
                .identity_metadata()
                .map_err(|_| PluginError::Unavailable)?,
            contest: self.contest.current_contest_id(),
        })
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
#[derive(Clone)]
struct Cursor {
    context: String,
    query: PluginSubmissionQuery,
    native_page: u32,
    pending: VecDeque<PluginSubmission>,
    more: bool,
    total: Option<u64>,
    created: Instant,
}
#[derive(Default)]
struct State {
    contexts: HashMap<String, Context>,
    cursors: HashMap<String, Cursor>,
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
        }
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
    pub fn check(&self, handle: &PluginQueryContext) -> PluginResult<CancellationToken> {
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
            || context.cancellation.is_cancelled()
            || self.source.scope()? != context.scope
            || context.cancellation.is_cancelled()
        {
            return Err(PluginError::ContextExpired);
        }
        Ok(context.cancellation)
    }
    fn context(&self, handle: &PluginQueryContext) -> PluginResult<Context> {
        self.check(handle)?;
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
        let snapshot = self.access.snapshot().ok_or(PluginError::Unavailable)?;
        let scope = self.source.scope()?;
        if snapshot.cancellation.is_cancelled() {
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
        let context = self.context(handle)?;
        Ok(context.scope.user.map(|(id, _)| {
            let key =
                serde_json::to_vec(&(&context.scope.oj, &context.scope.endpoint, id)).unwrap();
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
        let context = self.context(handle)?;
        let Some(contest) = context.scope.contest else {
            return Ok(None);
        };
        if id.is_some_and(|id| id != contest) {
            return Ok(None);
        }
        let cancellation = self.check(handle)?;
        let result = tokio::select! { biased;
            _ = self.stopped.cancelled() => return Err(PluginError::ContextExpired),
            _ = cancellation.cancelled() => return Err(PluginError::ContextExpired),
            result = self.source.contest(&contest) => result,
        };
        self.check(handle)?;
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
        let context = self.context(handle)?;
        let (_, username) = context.scope.user.ok_or(PluginError::Unauthenticated)?;
        if context.scope.contest.as_deref() != Some(&query.contest_id) {
            return Err(PluginError::ContextExpired);
        }
        let mut cursor = if let Some(id) = &query.cursor {
            let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            let c = state
                .cursors
                .get(id)
                .cloned()
                .ok_or(PluginError::ContextExpired)?;
            if c.context != handle.host_id()
                || c.query.contest_id != query.contest_id
                || c.query.problem_id != query.problem_id
                || c.query.limit != query.limit
                || c.created.elapsed() > Duration::from_secs(300)
            {
                return Err(PluginError::ContextExpired);
            }
            c
        } else {
            Cursor {
                context: handle.host_id().into(),
                query: query.clone(),
                native_page: 1,
                pending: VecDeque::new(),
                more: true,
                total: None,
                created: Instant::now(),
            }
        };
        if cursor.pending.is_empty() && cursor.more {
            if cursor.native_page > 10_000 {
                return Err(PluginError::LimitExceeded);
            }
            let cancellation = self.check(handle)?;
            let result = tokio::select! { biased;
                _ = self.stopped.cancelled() => return Err(PluginError::ContextExpired),
                _ = cancellation.cancelled() => return Err(PluginError::ContextExpired),
                result = self.source.history(&query.contest_id, cursor.native_page) => result,
            };
            self.check(handle)?;
            let page = result?;
            if page.records.len() > 1000 || page.records.iter().any(|r| r.username != username) {
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
            if serde_json::to_vec(&cursor.pending)
                .map_err(|_| PluginError::ProviderError)?
                .len()
                > 1024 * 1024
            {
                return Err(PluginError::LimitExceeded);
            }
        }
        let records = cursor
            .pending
            .drain(..cursor.pending.len().min(query.limit as usize))
            .collect();
        let next = if cursor.more || !cursor.pending.is_empty() {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            state
                .cursors
                .retain(|_, c| c.created.elapsed() < Duration::from_secs(300));
            if state.cursors.len() >= 32 {
                return Err(PluginError::LimitExceeded);
            }
            let id = uuid::Uuid::new_v4().to_string();
            state.cursors.insert(id.clone(), cursor.clone());
            Some(id)
        } else {
            None
        };
        if let Some(id) = query.cursor {
            self.state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .cursors
                .remove(&id);
        }
        self.check(handle)?;
        Ok(PluginSubmissionPage {
            records,
            total: cursor.total,
            next_cursor: next,
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
        let mut query = PluginSubmissionQuery {
            contest_id: contest.into(),
            problem_id: None,
            cursor: None,
            limit: 100,
        };
        for _ in 0..100 {
            let page = self.list(handle, query.clone()).await?;
            if let Some(record) = page.records.into_iter().find(|r| r.submission_id == id) {
                if let Some(cursor) = page.next_cursor {
                    self.state
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .cursors
                        .remove(&cursor);
                }
                return Ok(Some(record));
            }
            let Some(cursor) = page.next_cursor else {
                return Ok(None);
            };
            query.cursor = Some(cursor);
        }
        if let Some(cursor) = query.cursor {
            self.state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .cursors
                .remove(&cursor);
        }
        Err(PluginError::LimitExceeded)
    }
}
