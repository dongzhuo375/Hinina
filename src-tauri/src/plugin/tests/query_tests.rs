use super::support::*;
use crate::{
    core::{access::AccessRevision, entity::submission::SubmissionHistoryPage},
    plugin::{
        api::{submission::*, *},
        host::manifest::PluginPermission,
        query::ScopedQueries,
    },
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
        fn scope(&self) -> PluginResult<crate::plugin::query::QueryScope> {
            if self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                drop(self.access.begin());
            }
            crate::plugin::query::QuerySource::scope(&self.source)
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
        fn scope(&self) -> PluginResult<crate::plugin::query::QueryScope> {
            std::thread::sleep(std::time::Duration::from_millis(100));
            crate::plugin::query::QuerySource::scope(&Source::default())
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
    struct Sessions(AtomicUsize);
    impl SessionRepository for Sessions {
        fn load(&self, _: &OjId) -> AppResult<Option<Session>> {
            self.0.fetch_add(1, Ordering::SeqCst);
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
    let registry =
        Arc::new(ProviderRegistryImpl::new(OjId::new("HOJ")).with_access(access.clone()));
    let config = Arc::new(
        ConfigService::new(
            Arc::new(FsConfigRepository::new(storage.clone(), "config.json")),
            bus.clone(),
        )
        .with_access(access.clone()),
    );
    let endpoint = config
        .get()
        .oj
        .instances
        .into_iter()
        .find(|i| i.id == "HOJ")
        .unwrap()
        .base_url;
    registry.set_instance_endpoint(OjId::new("HOJ"), endpoint);
    let sessions = Arc::new(Sessions(AtomicUsize::new(0)));
    let auth = Arc::new(
        AuthService::new(registry.clone(), sessions.clone(), bus.clone())
            .with_access(access.clone()),
    );
    let source = Arc::new(ServiceQueries {
        auth: auth.clone(),
        contest: Arc::new(
            ContestService::new(registry.clone(), bus.clone(), storage.clone())
                .with_access(access.clone()),
        ),
        submission: Arc::new(SubmissionService::new(registry.clone(), bus, storage)),
        registry,
        config,
        access: access.clone(),
        cached_scope: Mutex::default(),
    });
    let m = manifest("test.cached");
    let q = ScopedQueries::new(
        crate::plugin::permission::PluginGrant::authorize(&m, &m.permissions).unwrap(),
        source,
        access,
        Default::default(),
    );
    let c = q.current_context().await.unwrap();
    for _ in 0..20 {
        assert!(q.identity(&c).await.unwrap().is_some());
    }
    assert_eq!(sessions.0.load(Ordering::SeqCst), 1);
    auth.logout().await.unwrap();
    assert!(matches!(
        q.check(&c).await,
        Err(PluginError::ContextExpired)
    ));
    let fresh = q.current_context().await.unwrap();
    assert!(q.identity(&fresh).await.unwrap().is_none());
    assert_eq!(sessions.0.load(Ordering::SeqCst), 1);
}
