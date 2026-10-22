use super::*;
use crate::plugin::test_support::*;
use crate::{
    core::{access::AccessRevision, entity::submission::SubmissionHistoryPage},
    plugin::{host::manifest::PluginPermission, query::ScopedQueries},
};
use std::sync::Arc;

#[tokio::test]
async fn native_remainders_and_unknown_total_never_claim_early_completion() {
    let source = Arc::new(Source::default());
    *source.pages.lock().unwrap() = vec![SubmissionHistoryPage {
        records: vec![record("1"), record("2"), record("3")],
        total: None,
        has_more: true,
    }];
    let access = Arc::new(AccessRevision::default());
    let m = manifest("test.query");
    let q = ScopedQueries::new(
        crate::plugin::permission::PluginGrant::authorize(&m, &m.permissions).unwrap(),
        source,
        access.clone(),
        Default::default(),
    );
    let c = q.current_context().await.unwrap();
    let mut query = PluginSubmissionQuery {
        contest_id: "contest".into(),
        problem_id: None,
        cursor: None,
        limit: 2,
    };
    let a = q.list(&c, query.clone()).await.unwrap();
    assert_eq!(a.records.len(), 2);
    assert_eq!(a.total, None);
    query.cursor = a.next_cursor;
    let b = q.list(&c, query.clone()).await.unwrap();
    assert_eq!(b.records[0].submission_id, "3");
    assert!(b.next_cursor.is_some());
    query.cursor = b.next_cursor;
    assert!(q.list(&c, query).await.unwrap().next_cursor.is_none());
    drop(access.begin());
    assert!(matches!(
        q.check(&c).await,
        Err(PluginError::ContextExpired)
    ));
}
#[tokio::test]
async fn in_flight_change_and_wrong_owner_fail_closed() {
    let source = Arc::new(Source {
        delay: true,
        ..Default::default()
    });
    let access = Arc::new(AccessRevision::default());
    let m = manifest("test.query");
    let q = ScopedQueries::new(
        crate::plugin::permission::PluginGrant::authorize(&m, &m.permissions).unwrap(),
        source.clone(),
        access.clone(),
        Default::default(),
    );
    let c = q.current_context().await.unwrap();
    let query = PluginSubmissionQuery {
        contest_id: "contest".into(),
        problem_id: None,
        cursor: None,
        limit: 100,
    };
    let (result, ()) = tokio::join!(q.list(&c, query.clone()), async {
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        drop(access.begin());
    });
    assert_eq!(result, Err(PluginError::ContextExpired));
    let mut wrong = record("1");
    wrong.owner_id = Some("other-user".into());
    *source.pages.lock().unwrap() = vec![SubmissionHistoryPage {
        records: vec![wrong],
        total: Some(1),
        has_more: false,
    }];
    let c = q.current_context().await.unwrap();
    assert_eq!(q.list(&c, query).await, Err(PluginError::ProviderError));
}
#[tokio::test]
async fn records_without_a_stable_owner_are_rejected_rather_than_trusted() {
    let source = Arc::new(Source::default());
    let mut ownerless = record("1");
    ownerless.owner_id = None;
    *source.pages.lock().unwrap() = vec![SubmissionHistoryPage {
        records: vec![ownerless],
        total: Some(1),
        has_more: false,
    }];
    let m = manifest("test.ownerless");
    let q = ScopedQueries::new(
        crate::plugin::permission::PluginGrant::authorize(&m, &m.permissions).unwrap(),
        source,
        Arc::default(),
        Default::default(),
    );
    let context = q.current_context().await.unwrap();
    assert_eq!(
        q.list(
            &context,
            PluginSubmissionQuery {
                contest_id: "contest".into(),
                problem_id: None,
                cursor: None,
                limit: 100,
            },
        )
        .await,
        Err(PluginError::ProviderError)
    );
}

#[tokio::test]
async fn permissions_and_context_cursor_bindings_are_checked() {
    let q = ScopedQueries::new(
        grant_without(PluginPermission::SubmissionRead),
        Arc::new(Source::default()),
        Arc::default(),
        Default::default(),
    );
    let c = q.current_context().await.unwrap();
    assert!(matches!(
        q.submission(&c, "contest", "1").await,
        Err(PluginError::PermissionDenied { .. })
    ));
    assert!(q
        .check(&PluginQueryContext::from_host_id("forged".into()))
        .await
        .is_err());
}

#[tokio::test]
async fn repeated_metadata_lookup_releases_unused_continuations() {
    let source = Arc::new(Source::default());
    *source.pages.lock().unwrap() = vec![SubmissionHistoryPage {
        records: vec![record("wanted")],
        total: None,
        has_more: true,
    }];
    let m = manifest("test.lookup");
    let q = ScopedQueries::new(
        crate::plugin::permission::PluginGrant::authorize(&m, &m.permissions).unwrap(),
        source,
        Arc::default(),
        Default::default(),
    );
    let context = q.current_context().await.unwrap();
    for _ in 0..40 {
        assert!(q
            .submission(&context, "contest", "wanted")
            .await
            .unwrap()
            .is_some());
    }
}

#[tokio::test]
async fn transition_during_scope_snapshot_cannot_return_a_revoked_token() {
    struct RevokeDuringSnapshot {
        source: Source,
        access: Arc<AccessRevision>,
        calls: std::sync::atomic::AtomicUsize,
    }
    #[async_trait::async_trait]
    impl crate::plugin::query::QuerySource for RevokeDuringSnapshot {
        fn scope(&self, plugin_id: &str) -> PluginResult<crate::plugin::query::QueryScope> {
            if self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                drop(self.access.begin());
            }
            crate::plugin::query::QuerySource::scope(&self.source, plugin_id)
        }
        async fn contest(&self, _: &str) -> PluginResult<crate::core::entity::contest::Contest> {
            Err(PluginError::Unavailable)
        }
        async fn history(&self, _: &str, _: u32) -> PluginResult<SubmissionHistoryPage> {
            Err(PluginError::Unavailable)
        }
    }
    let access = Arc::new(AccessRevision::default());
    let m = manifest("test.race");
    let source = Arc::new(RevokeDuringSnapshot {
        source: Source::default(),
        access: access.clone(),
        calls: Default::default(),
    });
    let q = ScopedQueries::new(
        crate::plugin::permission::PluginGrant::authorize(&m, &m.permissions).unwrap(),
        source,
        access,
        Default::default(),
    );
    let context = q.current_context().await.unwrap();
    assert!(matches!(
        q.check(&context).await,
        Err(PluginError::ContextExpired)
    ));
}

#[tokio::test]
async fn scope_failure_expires_existing_contexts_but_preserves_new_context_errors() {
    let source = Arc::new(Source::default());
    let m = manifest("test.scope-failure");
    let q = ScopedQueries::new(
        crate::plugin::permission::PluginGrant::authorize(&m, &m.permissions).unwrap(),
        source.clone(),
        Arc::default(),
        Default::default(),
    );
    let context = q.current_context().await.unwrap();
    for error in [PluginError::Unavailable, PluginError::ProviderError] {
        *source.scope_error.lock().unwrap() = Some(error.clone());
        assert!(matches!(
            q.check(&context).await,
            Err(PluginError::ContextExpired)
        ));
        assert_eq!(q.identity(&context).await, Err(PluginError::ContextExpired));
        assert_eq!(
            q.contest(&context, None).await,
            Err(PluginError::ContextExpired)
        );
        assert_eq!(q.current_context().await.unwrap_err(), error);
    }
}

#[tokio::test]
async fn scope_failure_during_query_rejects_the_completed_result() {
    let source = Arc::new(Source {
        delay: true,
        ..Default::default()
    });
    let m = manifest("test.scope-failure");
    let q = ScopedQueries::new(
        crate::plugin::permission::PluginGrant::authorize(&m, &m.permissions).unwrap(),
        source.clone(),
        Arc::default(),
        Default::default(),
    );
    let context = q.current_context().await.unwrap();
    let query = PluginSubmissionQuery {
        contest_id: "contest".into(),
        problem_id: None,
        cursor: None,
        limit: 100,
    };
    let (result, ()) = tokio::join!(q.list(&context, query), async {
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        *source.scope_error.lock().unwrap() = Some(PluginError::Unavailable);
    });
    assert_eq!(result, Err(PluginError::ContextExpired));
}

fn queries_with_budget(
    source: Arc<Source>,
    budget: Arc<crate::plugin::query::QueryBudget>,
) -> ScopedQueries {
    let m = manifest("test.budget");
    ScopedQueries::new(
        crate::plugin::permission::PluginGrant::authorize(&m, &m.permissions).unwrap(),
        source,
        Arc::default(),
        Default::default(),
    )
    .with_shared_budget(budget)
}

#[tokio::test]
async fn every_scanned_page_consumes_budget_and_new_activation_does_not_reset_shared_budget() {
    use crate::plugin::query::QueryBudget;
    use std::sync::atomic::Ordering;
    let source = Arc::new(Source::default());
    *source.pages.lock().unwrap() = vec![
        SubmissionHistoryPage {
            records: vec![record("other")],
            total: None,
            has_more: true,
        };
        10
    ];
    let budget = Arc::new(QueryBudget::new(3, 4));
    let q = queries_with_budget(source.clone(), budget.clone());
    let context = q.current_context().await.unwrap();
    // Contest and history share one budget; get must charge every page rather than one JS call.
    q.contest(&context, None).await.unwrap();
    assert_eq!(
        q.submission(&context, "contest", "missing").await,
        Err(PluginError::LimitExceeded)
    );
    assert_eq!(source.contest_calls.load(Ordering::SeqCst), 1);
    assert_eq!(source.history_calls.load(Ordering::SeqCst), 2);
    // A new context and a new activation do not reset the manager-wide OJ budget.
    let reloaded = queries_with_budget(source.clone(), budget);
    let fresh = reloaded.current_context().await.unwrap();
    assert_eq!(
        reloaded.contest(&fresh, None).await,
        Err(PluginError::LimitExceeded)
    );
    assert_eq!(source.contest_calls.load(Ordering::SeqCst), 1);
    // Limiting does not turn a missing lookup into false absence or disable local identity reads.
    assert!(reloaded.identity(&fresh).await.unwrap().is_some());
}

#[tokio::test]
async fn concurrent_queries_fail_fast_and_release_capacity_after_completion() {
    use crate::plugin::query::QueryBudget;
    use std::sync::atomic::Ordering;
    let source = Arc::new(Source {
        delay: true,
        ..Default::default()
    });
    let q = queries_with_budget(source.clone(), Arc::new(QueryBudget::new(120, 4)));
    let context = q.current_context().await.unwrap();
    let (a, b, c) = tokio::join!(
        q.contest(&context, None),
        q.contest(&context, None),
        q.contest(&context, None),
    );
    let results = [a, b, c];
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 2);
    assert_eq!(
        results
            .iter()
            .filter(|r| **r == Err(PluginError::LimitExceeded))
            .count(),
        1
    );
    assert_eq!(source.contest_calls.load(Ordering::SeqCst), 2);
    q.contest(&context, None).await.unwrap();
    assert_eq!(source.contest_calls.load(Ordering::SeqCst), 3);
}

#[tokio::test(start_paused = true)]
async fn provider_timeout_releases_permits_for_other_activations() {
    use crate::plugin::query::QueryBudget;
    use std::sync::atomic::Ordering;
    let budget = Arc::new(QueryBudget::new(120, 1));
    let source = Arc::new(Source {
        pending: true,
        ..Default::default()
    });
    let q = queries_with_budget(source.clone(), budget.clone());
    let context = q.current_context().await.unwrap();
    assert_eq!(q.contest(&context, None).await, Err(PluginError::Timeout));
    assert_eq!(source.contest_calls.load(Ordering::SeqCst), 1);
    let healthy = queries_with_budget(Arc::new(Source::default()), budget);
    let fresh = healthy.current_context().await.unwrap();
    healthy.contest(&fresh, None).await.unwrap();
}

#[tokio::test]
async fn slow_scope_io_does_not_block_the_tokio_executor() {
    struct SlowScope;
    #[async_trait::async_trait]
    impl crate::plugin::query::QuerySource for SlowScope {
        fn scope(&self, plugin_id: &str) -> PluginResult<crate::plugin::query::QueryScope> {
            std::thread::sleep(std::time::Duration::from_millis(100));
            crate::plugin::query::QuerySource::scope(&Source::default(), plugin_id)
        }
        async fn contest(&self, _: &str) -> PluginResult<crate::core::entity::contest::Contest> {
            unreachable!()
        }
        async fn history(&self, _: &str, _: u32) -> PluginResult<SubmissionHistoryPage> {
            unreachable!()
        }
    }
    let m = manifest("test.blocking");
    let q = ScopedQueries::new(
        crate::plugin::permission::PluginGrant::authorize(&m, &m.permissions).unwrap(),
        Arc::new(SlowScope),
        Arc::default(),
        Default::default(),
    );
    // This is a single-thread executor: the timer wins only if scope runs elsewhere.
    let mut current = Box::pin(q.current_context());
    tokio::select! { biased;
        _ = &mut current => panic!("scope blocked the executor"),
        _ = tokio::time::sleep(std::time::Duration::from_millis(10)) => {},
    }
    let context = current.await.unwrap();
    let mut check = Box::pin(q.check(&context));
    tokio::select! { biased;
        _ = &mut check => panic!("context check blocked the executor"),
        _ = tokio::time::sleep(std::time::Duration::from_millis(10)) => {},
    }
    check.await.unwrap();
}

#[tokio::test]
async fn per_plugin_budget_limits_one_activation_without_exhausting_other_plugins() {
    use crate::plugin::query::QueryBudget;
    use std::sync::atomic::Ordering;
    let budget = Arc::new(QueryBudget::new(120, 4));
    let source = Arc::new(Source::default());
    let q = queries_with_budget(source.clone(), budget.clone());
    let context = q.current_context().await.unwrap();
    let mut limited = false;
    for _ in 0..80 {
        match q.contest(&context, None).await {
            Ok(_) => {}
            Err(PluginError::LimitExceeded) => {
                limited = true;
                break;
            }
            Err(error) => panic!("unexpected error: {error:?}"),
        }
    }
    assert!(limited);
    assert!(source.contest_calls.load(Ordering::SeqCst) >= 60);
    let other = queries_with_budget(Arc::new(Source::default()), budget);
    let context = other.current_context().await.unwrap();
    other.contest(&context, None).await.unwrap();
}

#[tokio::test]
async fn current_context_reports_expiration_throughout_a_transition() {
    let access = Arc::new(AccessRevision::default());
    let m = manifest("test.transition");
    let q = ScopedQueries::new(
        crate::plugin::permission::PluginGrant::authorize(&m, &m.permissions).unwrap(),
        Arc::new(Source::default()),
        access.clone(),
        Default::default(),
    );
    let old = q.current_context().await.unwrap();
    let transition = access.begin();
    assert_eq!(q.current_context().await, Err(PluginError::ContextExpired));
    assert!(matches!(
        q.check(&old).await,
        Err(PluginError::ContextExpired)
    ));
    drop(transition);
    q.current_context().await.unwrap();
    assert!(matches!(
        q.check(&old).await,
        Err(PluginError::ContextExpired)
    ));
}

#[tokio::test]
async fn concurrent_cursor_reuse_has_only_one_winner() {
    let source = Arc::new(Source {
        delay: true,
        ..Default::default()
    });
    *source.pages.lock().unwrap() = vec![SubmissionHistoryPage {
        records: vec![record("1"), record("2"), record("3")],
        total: None,
        has_more: false,
    }];
    let m = manifest("test.cursor");
    let q = ScopedQueries::new(
        crate::plugin::permission::PluginGrant::authorize(&m, &m.permissions).unwrap(),
        source,
        Arc::default(),
        Default::default(),
    );
    let c = q.current_context().await.unwrap();
    let mut query = PluginSubmissionQuery {
        contest_id: "contest".into(),
        problem_id: None,
        cursor: None,
        limit: 1,
    };
    query.cursor = q.list(&c, query.clone()).await.unwrap().next_cursor;
    let (a, b) = tokio::join!(q.list(&c, query.clone()), q.list(&c, query));
    let results = [a, b];
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| **r == Err(PluginError::ContextExpired))
            .count(),
        1
    );
    let winner = results.into_iter().find_map(Result::ok).unwrap();
    assert_eq!(winner.records[0].submission_id, "2");
}

#[tokio::test]
async fn production_scope_reads_session_once_per_revision_and_refreshes_after_revocation() {
    use crate::{
        core::{
            entity::session::Session,
            error::AppResult,
            event::core_event_bus::CoreEventBus,
            provider::{oj_id::OjId, registry::ProviderRegistry},
            repository::session_repo::SessionRepository,
        },
        infra::{
            fs_config_repo::FsConfigRepository, provider_registry_impl::ProviderRegistryImpl,
            storage::Storage,
        },
        plugin::query::ServiceQueries,
        service::{
            auth::AuthService, config::ConfigService, contest::ContestService,
            submission::SubmissionService,
        },
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    };
    struct Sessions {
        loads: AtomicUsize,
        block: Mutex<
            Option<(
                tokio::sync::oneshot::Sender<()>,
                std::sync::mpsc::Receiver<()>,
            )>,
        >,
    }
    impl SessionRepository for Sessions {
        fn load(&self, _: &OjId) -> AppResult<Option<Session>> {
            self.loads.fetch_add(1, Ordering::SeqCst);
            let block = self.block.lock().unwrap().take();
            if let Some((reached, release)) = block {
                reached.send(()).unwrap();
                release
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .unwrap();
            }
            Ok(Some(Session::new("HOJ", "user-1", "alice", "private")))
        }
        fn save(&self, _: &Session) -> AppResult<()> {
            unreachable!()
        }
        fn remove(&self, _: &OjId) -> AppResult<()> {
            Ok(())
        }
        fn rotate_token(&self, _: &OjId, _: &str) -> AppResult<bool> {
            unreachable!()
        }
    }
    let dir = crate::test_support::TempDir::unique("scope-cache");
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let bus = Arc::new(CoreEventBus::new());
    let access = Arc::new(AccessRevision::default());
    let registry = Arc::new(ProviderRegistryImpl::new(OjId::new("HOJ"), access.clone()));
    let config = Arc::new(ConfigService::new(
        Arc::new(FsConfigRepository::new(storage.clone(), "config.json")),
        bus.clone(),
        access.clone(),
    ));
    let endpoint = config
        .get()
        .oj
        .instances
        .into_iter()
        .find(|i| i.id == "HOJ")
        .unwrap()
        .base_url;
    registry.set_instance_endpoint(OjId::new("HOJ"), endpoint);
    let sessions = Arc::new(Sessions {
        loads: AtomicUsize::new(0),
        block: Mutex::default(),
    });
    let auth = Arc::new(AuthService::new(
        registry.clone(),
        sessions.clone(),
        bus.clone(),
        access.clone(),
    ));
    let source = Arc::new(ServiceQueries {
        auth: auth.clone(),
        contest: Arc::new(ContestService::new(
            registry.clone(),
            bus.clone(),
            storage.clone(),
            access.clone(),
        )),
        submission: Arc::new(SubmissionService::new(registry.clone(), bus, storage)),
        registry,
        config,
        access: access.clone(),
        cached_scope: Mutex::default(),
        scope_warnings: Default::default(),
    });
    let m = manifest("test.cached");
    let q = ScopedQueries::new(
        crate::plugin::permission::PluginGrant::authorize(&m, &m.permissions).unwrap(),
        source.clone(),
        access.clone(),
        Default::default(),
    );
    let c = q.current_context().await.unwrap();
    for _ in 0..20 {
        assert!(q.identity(&c).await.unwrap().is_some());
    }
    assert_eq!(sessions.loads.load(Ordering::SeqCst), 1);
    // Force an old cache miss to finish after a new revision's reader. The new
    // reader must not wait for the old session I/O or receive the old snapshot.
    use crate::plugin::query::QuerySource;
    let mut stale = source.cached_scope.lock().unwrap().clone().unwrap();
    drop(access.begin());
    let (reached, waiting) = tokio::sync::oneshot::channel();
    let (release, blocked) = std::sync::mpsc::channel();
    *sessions.block.lock().unwrap() = Some((reached, blocked));
    let old_reader = {
        let source = source.clone();
        tokio::task::spawn_blocking(move || source.scope("test.cached"))
    };
    tokio::time::timeout(std::time::Duration::from_secs(2), waiting)
        .await
        .unwrap()
        .unwrap();
    drop(access.begin());
    let new_reader = {
        let source = source.clone();
        tokio::task::spawn_blocking(move || source.scope("test.cached"))
    };
    let current = tokio::time::timeout(std::time::Duration::from_secs(2), new_reader)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    release.send(()).unwrap();
    assert!(matches!(
        old_reader.await.unwrap(),
        Err(PluginError::ContextExpired)
    ));
    assert!(source.scope("test.cached").unwrap() == current);
    assert_eq!(sessions.loads.load(Ordering::SeqCst), 3);
    // Simulate the numeric ABA after u64 wrap: a revoked token must never hit
    // even when its revision is numerically equal to the current revision.
    stale.0.revision = access.snapshot().unwrap().revision;
    stale.1.user = Some(("stale-owner".into(), "stale-name".into()));
    *source.cached_scope.lock().unwrap() = Some(stale);
    // The no-I/O fast path must reject the numerically-equal-but-revoked snapshot too,
    // not only the blocking slow path asserted below.
    assert!(source.cached_scope().is_none());
    assert!(source.scope("test.cached").unwrap() == current);
    assert_eq!(sessions.loads.load(Ordering::SeqCst), 4);
    auth.logout().await.unwrap();
    assert!(matches!(
        q.check(&c).await,
        Err(PluginError::ContextExpired)
    ));
    let fresh = q.current_context().await.unwrap();
    assert!(q.identity(&fresh).await.unwrap().is_none());
    assert_eq!(sessions.loads.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn cursor_memory_budget_is_shared_and_released_when_continuations_are_consumed() {
    let source = Arc::new(Source::default());
    let mut large = record("2");
    large.display_id = "x".repeat(850_000);
    *source.pages.lock().unwrap() = vec![SubmissionHistoryPage {
        records: vec![record("1"), large],
        total: None,
        has_more: false,
    }];
    let m = manifest("test.cursor-bytes");
    let q = ScopedQueries::new(
        crate::plugin::permission::PluginGrant::authorize(&m, &m.permissions).unwrap(),
        source.clone(),
        Arc::default(),
        Default::default(),
    );
    let context = q.current_context().await.unwrap();
    let query = PluginSubmissionQuery {
        contest_id: "contest".into(),
        problem_id: None,
        cursor: None,
        limit: 1,
    };
    let mut continuations = Vec::new();
    for _ in 0..4 {
        continuations.push(
            q.list(&context, query.clone())
                .await
                .unwrap()
                .next_cursor
                .unwrap(),
        );
    }
    assert_eq!(
        q.list(&context, query.clone()).await,
        Err(PluginError::LimitExceeded)
    );
    let remainder = q
        .list(
            &context,
            PluginSubmissionQuery {
                cursor: continuations.pop(),
                ..query.clone()
            },
        )
        .await
        .unwrap();
    assert_eq!(remainder.records[0].submission_id, "2");
    assert!(remainder.next_cursor.is_none());
    assert!(q
        .list(&context, query.clone())
        .await
        .unwrap()
        .next_cursor
        .is_some());
    // A single oversized native projection is rejected before retaining it.
    source.pages.lock().unwrap()[0].records[1].display_id = "x".repeat(1024 * 1024);
    assert_eq!(
        q.list(&context, query).await,
        Err(PluginError::LimitExceeded)
    );
}

#[tokio::test]
async fn a_failed_page_fetch_keeps_the_cursor_for_a_retry() {
    let source = Arc::new(Source::default());
    let mut intruder = record("2");
    intruder.owner_id = Some("someone-else".into());
    *source.pages.lock().unwrap() = vec![
        SubmissionHistoryPage {
            records: vec![record("1")],
            total: None,
            has_more: true,
        },
        SubmissionHistoryPage {
            records: vec![intruder],
            total: None,
            has_more: false,
        },
    ];
    let m = manifest("test.retry-cursor");
    let q = ScopedQueries::new(
        crate::plugin::permission::PluginGrant::authorize(&m, &m.permissions).unwrap(),
        source.clone(),
        Arc::default(),
        Default::default(),
    );
    let context = q.current_context().await.unwrap();
    let query = PluginSubmissionQuery {
        contest_id: "contest".into(),
        problem_id: None,
        cursor: None,
        limit: 1,
    };
    let cursor = q
        .list(&context, query.clone())
        .await
        .unwrap()
        .next_cursor
        .unwrap();
    // The page fails validation after the cursor was consumed; the caller must be able to
    // retry with the same cursor instead of restarting from the first page.
    assert_eq!(
        q.list(
            &context,
            PluginSubmissionQuery {
                cursor: Some(cursor.clone()),
                ..query.clone()
            },
        )
        .await,
        Err(PluginError::ProviderError)
    );
    source.pages.lock().unwrap()[1].records = vec![record("2")];
    let resumed = q
        .list(
            &context,
            PluginSubmissionQuery {
                cursor: Some(cursor),
                ..query
            },
        )
        .await
        .unwrap();
    assert_eq!(resumed.records[0].submission_id, "2");
    assert!(resumed.next_cursor.is_none());
}

#[tokio::test]
async fn an_oversized_page_restores_the_true_precall_cursor_snapshot() {
    let source = Arc::new(Source::default());
    // Page 1 publishes a cursor; page 2 is oversized, so advance() fails at the encoded-size
    // check AFTER it would otherwise have mutated the cursor. The restore must put back the
    // true pre-call snapshot (empty pending, unchanged native_page), not the oversized page
    // with stale buffered_bytes — which would bypass the per-cursor and aggregate budgets.
    let mut oversized = record("2");
    oversized.display_id = "x".repeat(1024 * 1024);
    *source.pages.lock().unwrap() = vec![
        SubmissionHistoryPage {
            records: vec![record("1")],
            total: None,
            has_more: true,
        },
        SubmissionHistoryPage {
            records: vec![oversized],
            total: None,
            has_more: false,
        },
    ];
    let m = manifest("test.oversized-restore");
    let q = ScopedQueries::new(
        crate::plugin::permission::PluginGrant::authorize(&m, &m.permissions).unwrap(),
        source.clone(),
        Arc::default(),
        Default::default(),
    );
    let context = q.current_context().await.unwrap();
    let query = PluginSubmissionQuery {
        contest_id: "contest".into(),
        problem_id: None,
        cursor: None,
        limit: 1,
    };
    let cursor = q
        .list(&context, query.clone())
        .await
        .unwrap()
        .next_cursor
        .unwrap();
    // The next native page is oversized: rejected, and the cursor must survive uncorrupted.
    assert_eq!(
        q.list(
            &context,
            PluginSubmissionQuery {
                cursor: Some(cursor.clone()),
                ..query.clone()
            },
        )
        .await,
        Err(PluginError::LimitExceeded)
    );
    // Heal the page. A retry with the same cursor must resume from page 2, proving the
    // restored cursor kept its true pre-call state instead of retaining the oversized
    // records (which would make this retry fail again, and would leave >1MiB resident
    // while the aggregate accounting still counted only the stale buffered_bytes).
    source.pages.lock().unwrap()[1].records = vec![record("2")];
    let resumed = q
        .list(
            &context,
            PluginSubmissionQuery {
                cursor: Some(cursor),
                ..query
            },
        )
        .await
        .unwrap();
    assert_eq!(resumed.records[0].submission_id, "2");
    assert!(resumed.next_cursor.is_none());
}

#[tokio::test]
async fn cached_scope_hits_never_touch_the_blocking_read() {
    use crate::plugin::query::{QueryScope, QuerySource};
    struct Cached;
    #[async_trait::async_trait]
    impl QuerySource for Cached {
        fn scope(&self, _: &str) -> PluginResult<QueryScope> {
            panic!("a cache hit must not perform the blocking scope read")
        }
        fn cached_scope(&self) -> Option<QueryScope> {
            Some(QueryScope {
                oj: "hoj".into(),
                endpoint: "https://oj.example.com".into(),
                user: Some(("user-1".into(), "alice".into())),
                contest: Some("contest".into()),
            })
        }
        async fn contest(&self, _: &str) -> PluginResult<Contest> {
            unreachable!()
        }
        async fn history(&self, _: &str, _: u32) -> PluginResult<SubmissionHistoryPage> {
            unreachable!()
        }
    }
    let m = manifest("test.cached-hot");
    let q = ScopedQueries::new(
        crate::plugin::permission::PluginGrant::authorize(&m, &m.permissions).unwrap(),
        Arc::new(Cached),
        Arc::default(),
        Default::default(),
    );
    let context = q.current_context().await.unwrap();
    // Each bridge call checks the scope two or three times; all hits stay off the pool.
    for _ in 0..5 {
        q.check(&context).await.unwrap();
    }
}

#[test]
fn scope_cache_hit_requires_matching_revision_and_live_tokens() {
    use crate::core::access::AccessSnapshot;
    use tokio_util::sync::CancellationToken;
    fn snapshot(revision: u64, cancellation: CancellationToken) -> AccessSnapshot {
        AccessSnapshot {
            revision,
            cancellation,
        }
    }
    // This predicate is the single source of truth shared by the no-I/O fast path
    // (`cached_scope`) and the blocking slow path (`scope`). Locking it here means a
    // revoked token can never be served from cache after an OJ/account/contest switch,
    // and the two paths cannot drift apart.
    let live = CancellationToken::new();
    let cached = CancellationToken::new();
    // Hit: same revision and neither token cancelled.
    assert!(scope_cache_hit(
        &snapshot(7, cached.clone()),
        &snapshot(7, live.clone())
    ));
    // Miss: revision differs.
    assert!(!scope_cache_hit(
        &snapshot(6, cached.clone()),
        &snapshot(7, live.clone())
    ));
    // Miss: the cached token was revoked. This is the numeric-ABA guard — a revision that
    // wraps back to an equal value must still fail because the old token is cancelled.
    let revoked_cached = CancellationToken::new();
    revoked_cached.cancel();
    assert!(!scope_cache_hit(
        &snapshot(7, revoked_cached),
        &snapshot(7, live.clone())
    ));
    // Miss: the live snapshot token was revoked mid-transition.
    let revoked_live = CancellationToken::new();
    revoked_live.cancel();
    assert!(!scope_cache_hit(
        &snapshot(7, cached),
        &snapshot(7, revoked_live)
    ));
}

#[tokio::test]
async fn oversized_bridge_page_never_publishes_an_unreachable_cursor() {
    use crate::plugin::limits::{
        MAX_BRIDGE_RESPONSE_BYTES, MAX_CURSOR_PAGE_BYTES, MAX_CURSOR_RESPONSE_BYTES,
    };
    let source = Arc::new(Source::default());
    *source.pages.lock().unwrap() = vec![SubmissionHistoryPage {
        records: vec![record("1")],
        total: None,
        has_more: false,
    }];
    let m = manifest("test.page-bound");
    let q = ScopedQueries::new(
        crate::plugin::permission::PluginGrant::authorize(&m, &m.permissions).unwrap(),
        source.clone(),
        Arc::default(),
        Default::default(),
    );
    let context = q.current_context().await.unwrap();
    let query = PluginSubmissionQuery {
        contest_id: "contest".into(),
        problem_id: None,
        cursor: None,
        limit: 1,
    };
    let base = q.list(&context, query.clone()).await.unwrap();
    let mut probe = base.records[0].clone();
    probe.problem_display_id = String::new();
    let page_overhead = serde_json::to_string(&PluginSubmissionPage {
        records: vec![probe],
        total: None,
        next_cursor: Some("0".repeat(36)),
    })
    .unwrap()
    .len();
    // The page is just over the response bound, while the retained pending page is
    // still inside the cursor bound, so only the pre-publication check can reject it.
    let padding = MAX_BRIDGE_RESPONSE_BYTES - page_overhead - 10;
    assert!(padding + page_overhead > MAX_CURSOR_RESPONSE_BYTES);
    assert!(padding + page_overhead - 100 < MAX_CURSOR_PAGE_BYTES);
    let mut large = record("2");
    large.display_id = "x".repeat(padding);
    *source.pages.lock().unwrap() = vec![SubmissionHistoryPage {
        records: vec![large, record("3")],
        total: None,
        has_more: true,
    }];
    assert_eq!(
        q.list(&context, query).await,
        Err(PluginError::LimitExceeded)
    );
    assert!(q.state.lock().unwrap().cursors.is_empty());
}

#[tokio::test(start_paused = true)]
async fn metadata_scan_has_one_total_deadline_and_releases_query_resources() {
    use crate::plugin::limits::{ACTIVATION_TIMEOUT, API_ERROR_GRACE, SUBMISSION_LOOKUP_TIMEOUT};
    use std::{sync::atomic::Ordering, time::Duration};
    assert!(SUBMISSION_LOOKUP_TIMEOUT + API_ERROR_GRACE < ACTIVATION_TIMEOUT);
    let source = Arc::new(Source {
        history_delay: Some(Duration::from_secs(5)),
        ..Default::default()
    });
    *source.pages.lock().unwrap() = (0..10)
        .map(|_| SubmissionHistoryPage {
            records: vec![record("other")],
            total: None,
            has_more: true,
        })
        .collect();
    let q = Arc::new(queries_with_budget(
        source.clone(),
        Arc::new(crate::plugin::query::QueryBudget::new(120, 1)),
    ));
    let context = q.current_context().await.unwrap();
    let (result, ()) = tokio::join!(q.submission(&context, "contest", "missing"), async {
        source.history_started.notified().await;
        tokio::time::advance(Duration::from_secs(5)).await;
        source.history_started.notified().await;
        tokio::time::advance(Duration::from_secs(3)).await;
    });
    assert_eq!(result, Err(PluginError::Timeout));
    assert_eq!(source.history_calls.load(Ordering::SeqCst), 2);
    assert!(q.state.lock().unwrap().cursors.is_empty());
    assert_eq!(q.budget.concurrent.available_permits(), 2);
    assert_eq!(q.shared_budget.concurrent.available_permits(), 1);
    q.contest(&context, None).await.unwrap();
}

#[tokio::test]
async fn dropping_list_during_final_scope_check_removes_unpublished_continuation() {
    use crate::plugin::query::{QueryScope, QuerySource};
    use std::{
        sync::{
            atomic::{AtomicUsize, Ordering},
            Mutex,
        },
        time::Duration,
    };
    struct FinalCheck {
        reads: AtomicUsize,
        entered: tokio::sync::Notify,
        release: Mutex<std::sync::mpsc::Receiver<()>>,
    }
    #[async_trait::async_trait]
    impl QuerySource for FinalCheck {
        fn scope(&self, plugin_id: &str) -> PluginResult<QueryScope> {
            // context.current, list context, remote before/after, final publication check.
            if self.reads.fetch_add(1, Ordering::SeqCst) == 4 {
                self.entered.notify_one();
                self.release
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(5))
                    .unwrap();
            }
            Source::default().scope(plugin_id)
        }
        async fn contest(&self, _: &str) -> PluginResult<crate::core::entity::contest::Contest> {
            unreachable!()
        }
        async fn history(&self, _: &str, _: u32) -> PluginResult<SubmissionHistoryPage> {
            Ok(SubmissionHistoryPage {
                records: vec![record("1"), record("2")],
                total: None,
                has_more: false,
            })
        }
    }
    let (release, receiver) = std::sync::mpsc::channel();
    let source = Arc::new(FinalCheck {
        reads: AtomicUsize::new(0),
        entered: tokio::sync::Notify::new(),
        release: Mutex::new(receiver),
    });
    let m = manifest("test.cancel-page");
    let q = Arc::new(ScopedQueries::new(
        crate::plugin::permission::PluginGrant::authorize(&m, &m.permissions).unwrap(),
        source.clone(),
        Arc::default(),
        Default::default(),
    ));
    let c = q.current_context().await.unwrap();
    let operation = {
        let q = q.clone();
        tokio::spawn(async move {
            q.list(
                &c,
                PluginSubmissionQuery {
                    contest_id: "contest".into(),
                    problem_id: None,
                    cursor: None,
                    limit: 1,
                },
            )
            .await
        })
    };
    tokio::time::timeout(Duration::from_secs(2), source.entered.notified())
        .await
        .unwrap();
    assert_eq!(q.state.lock().unwrap().cursors.len(), 1);
    operation.abort();
    assert!(operation.await.unwrap_err().is_cancelled());
    assert!(q.state.lock().unwrap().cursors.is_empty());
    release.send(()).unwrap();
}

#[tokio::test]
async fn pending_endpoint_edit_keeps_running_provider_scope_and_emits_redacted_diagnostics() {
    use crate::{
        core::{
            entity::session::Session,
            event::core_event_bus::CoreEventBus,
            provider::{oj_id::OjId, registry::ProviderRegistry},
        },
        infra::{
            fs_config_repo::FsConfigRepository, provider_registry_impl::ProviderRegistryImpl,
            storage::Storage,
        },
        plugin::query::{QuerySource, ServiceQueries},
        service::{
            auth::AuthService, config::ConfigService, contest::ContestService,
            submission::SubmissionService,
        },
    };
    use std::{io::Write, sync::Mutex};
    use tracing::instrument::WithSubscriber;
    #[derive(Clone)]
    struct Capture(Arc<Mutex<Vec<u8>>>);
    impl Write for Capture {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(data);
            Ok(data.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let dir = crate::test_support::TempDir::unique("scope-pending-endpoint");
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let bus = Arc::new(CoreEventBus::new());
    let access = Arc::new(AccessRevision::default());
    let registry = Arc::new(ProviderRegistryImpl::new(OjId::new("HOJ"), access.clone()));
    let config = Arc::new(ConfigService::new(
        Arc::new(FsConfigRepository::new(storage.clone(), "config.json")),
        bus.clone(),
        access.clone(),
    ));
    let endpoint = config.get().oj.instances[0].base_url.clone();
    let deps = crate::adapter::test_adapter_deps(storage.clone());
    let factory = crate::adapter::factories()
        .into_iter()
        .find(|factory| factory.id() == "HOJ")
        .unwrap();
    registry.register(OjId::new("HOJ"), factory.build(&deps, &endpoint));
    registry.set_instance_endpoint(OjId::new("HOJ"), endpoint.clone());
    let provider = registry.current_submission().unwrap();
    deps.session_repo
        .save(&Session::new("HOJ", "user-1", "alice", "PRIVATE_TOKEN"))
        .unwrap();
    let source = Arc::new(ServiceQueries {
        auth: Arc::new(AuthService::new(
            registry.clone(),
            deps.session_repo,
            bus.clone(),
            access.clone(),
        )),
        contest: Arc::new(ContestService::new(
            registry.clone(),
            bus.clone(),
            storage.clone(),
            access.clone(),
        )),
        submission: Arc::new(SubmissionService::new(registry.clone(), bus, storage)),
        registry: registry.clone(),
        config: config.clone(),
        access: access.clone(),
        cached_scope: Mutex::default(),
        scope_warnings: Default::default(),
    });
    let m = manifest("test.endpoint");
    let q = ScopedQueries::new(
        crate::plugin::permission::PluginGrant::authorize(&m, &m.permissions).unwrap(),
        source.clone(),
        access,
        Default::default(),
    );
    let old = q.current_context().await.unwrap();
    let old_identity = q.identity(&old).await.unwrap();
    config
        .update(|cfg| {
            cfg.oj.instances[0].base_url =
                "https://pending.example.invalid/private?token=SECRET".into()
        })
        .unwrap();
    assert!(matches!(
        q.check(&old).await,
        Err(PluginError::ContextExpired)
    ));
    let _sink = tracing::Dispatch::new(
        tracing_subscriber::fmt()
            .with_writer(std::io::sink)
            .finish(),
    );
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let writer = Capture(bytes.clone());
    let dispatch = tracing::Dispatch::new(
        tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish(),
    );
    let current = q
        .current_context()
        .with_subscriber(dispatch.clone())
        .await
        .unwrap();
    for _ in 0..3 {
        assert_eq!(
            q.identity(&current)
                .with_subscriber(dispatch.clone())
                .await
                .unwrap(),
            old_identity
        );
    }
    assert_eq!(source.scope("test.endpoint").unwrap().endpoint, endpoint);
    assert!(Arc::ptr_eq(
        &provider,
        &registry.current_submission().unwrap()
    ));
    // A pending URL is different from revoking the instance itself.
    config
        .update(|cfg| cfg.oj.instances[0].enabled = false)
        .unwrap();
    assert_eq!(
        q.current_context().with_subscriber(dispatch.clone()).await,
        Err(PluginError::Unavailable)
    );
    config
        .update(|cfg| cfg.oj.instances[0].enabled = true)
        .unwrap();
    q.current_context().await.unwrap();
    let log = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
    assert_eq!(log.matches("endpoint_pending_restart").count(), 1);
    assert!(log.contains("instance_disabled_or_missing"));
    for secret in [
        "PRIVATE_TOKEN",
        "SECRET",
        "pending.example.invalid",
        endpoint.as_str(),
    ] {
        assert!(!log.contains(secret));
    }
}

#[tokio::test]
async fn provider_failures_are_logged_by_category_without_messages() {
    use crate::{
        core::{
            error::AppResult, event::core_event_bus::CoreEventBus, provider::oj_id::OjId,
            repository::session_repo::SessionRepository,
        },
        infra::{
            fs_config_repo::FsConfigRepository, provider_registry_impl::ProviderRegistryImpl,
            storage::Storage,
        },
        plugin::query::{QuerySource, ServiceQueries},
        service::{
            auth::AuthService, config::ConfigService, contest::ContestService,
            submission::SubmissionService,
        },
    };
    use std::io::Write;
    use std::sync::Mutex;
    #[derive(Clone)]
    struct Capture(Arc<Mutex<Vec<u8>>>);
    impl Write for Capture {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(data);
            Ok(data.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    struct Sessions;
    impl SessionRepository for Sessions {
        fn load(&self, _: &OjId) -> AppResult<Option<crate::core::entity::session::Session>> {
            Ok(None)
        }
        fn save(&self, _: &crate::core::entity::session::Session) -> AppResult<()> {
            Ok(())
        }
        fn remove(&self, _: &OjId) -> AppResult<()> {
            Ok(())
        }
        fn rotate_token(&self, _: &OjId, _: &str) -> AppResult<bool> {
            Ok(false)
        }
    }
    let dir = crate::test_support::TempDir::unique("query-provider-log");
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let bus = Arc::new(CoreEventBus::new());
    let access = Arc::new(AccessRevision::default());
    // No provider is registered, so both reads fail before any network call.
    let registry = Arc::new(ProviderRegistryImpl::new(OjId::new("HOJ"), access.clone()));
    let source = Arc::new(ServiceQueries {
        auth: Arc::new(AuthService::new(
            registry.clone(),
            Arc::new(Sessions),
            bus.clone(),
            access.clone(),
        )),
        contest: Arc::new(ContestService::new(
            registry.clone(),
            bus.clone(),
            storage.clone(),
            access.clone(),
        )),
        submission: Arc::new(SubmissionService::new(
            registry.clone(),
            bus.clone(),
            storage,
        )),
        registry,
        config: Arc::new(ConfigService::new(
            Arc::new(FsConfigRepository::new(
                Arc::new(Storage::new(dir.to_path_buf())),
                "config.json",
            )),
            bus,
            access.clone(),
        )),
        access,
        cached_scope: Mutex::default(),
        scope_warnings: Default::default(),
    });
    let _sink = tracing::Dispatch::new(
        tracing_subscriber::fmt()
            .with_writer(std::io::sink)
            .finish(),
    );
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let writer = Capture(bytes.clone());
    let dispatch = tracing::Dispatch::new(
        tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_max_level(tracing::Level::WARN)
            .with_writer(move || writer.clone())
            .finish(),
    );
    assert!(matches!(
        tracing::dispatcher::with_default(&dispatch, || source.scope("test.provider-log")),
        Err(PluginError::Unavailable)
    ));
    use tracing::instrument::WithSubscriber;
    assert_eq!(
        QuerySource::contest(&*source, "contest")
            .with_subscriber(dispatch.clone())
            .await
            .unwrap_err(),
        PluginError::ProviderError
    );
    assert_eq!(
        QuerySource::history(&*source, "contest", 1)
            .with_subscriber(dispatch)
            .await
            .unwrap_err(),
        PluginError::ProviderError
    );
    let log = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
    assert!(log.contains("contest.get"), "{log}");
    assert!(log.contains("submission.history"), "{log}");
    assert!(log.contains("provider_not_found"), "{log}");
    assert!(log.contains("oj_id"), "{log}");
    // Provider messages stay out of the log; only the stable category is recorded.
    assert!(!log.contains("未注册"), "{log}");
    assert!(!log.contains("未提供该能力"), "{log}");
}

#[test]
fn concurrency_rejections_never_spend_a_rate_token() {
    let local = QueryBudget::new(2, 1);
    let shared = QueryBudget::new(120, 1);
    let held = shared.concurrent.try_acquire().unwrap();
    for _ in 0..5 {
        assert!(QueryBudget::acquire_pair(&local, &shared).is_err());
    }
    drop(held);
    let held_local = local.concurrent.try_acquire().unwrap();
    for _ in 0..5 {
        assert!(QueryBudget::acquire_pair(&local, &shared).is_err());
    }
    drop(held_local);
    // Neither bucket was charged by the rejected attempts: the full burst of 2 remains.
    drop(QueryBudget::acquire_pair(&local, &shared).unwrap());
    drop(QueryBudget::acquire_pair(&local, &shared).unwrap());
    assert!(QueryBudget::acquire_pair(&local, &shared).is_err());
}
