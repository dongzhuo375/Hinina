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
    assert!(matches!(q.check(&c), Err(PluginError::ContextExpired)));
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
    wrong.username = "bob".into();
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
