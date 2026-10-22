use super::*;

fn manifest() -> PluginManifest {
    serde_json::from_value(serde_json::json!({
        "id":"com.example.demo", "name":"Demo", "version":"1.0.0",
        "manifestVersion":2, "apiVersion":2, "runtime":"js", "entry":"index.js",
        "permissions":["events.subscribe", "submission.read"], "events":["submission.judged"]
    }))
    .unwrap()
}

#[test]
fn declaration_is_not_an_authorization() {
    let grant = PluginGrant::authorize(&manifest(), &[PluginPermission::EventsSubscribe]).unwrap();
    assert!(grant.require(PluginPermission::EventsSubscribe).is_ok());
    assert_eq!(
        grant.require(PluginPermission::SubmissionRead),
        Err(PluginError::PermissionDenied {
            permission: PluginPermission::SubmissionRead
        })
    );
    assert!(PluginGrant::authorize(&manifest(), &[PluginPermission::NetworkRequest]).is_err());
}

#[test]
fn snapshot_is_immutable_and_invalid_manifest_cannot_be_authorized() {
    let mut source = manifest();
    let grant = PluginGrant::authorize(&source, &source.permissions).unwrap();
    source.id.clear();
    assert_eq!(grant.manifest().id, "com.example.demo");
    assert!(PluginGrant::authorize(&source, &[]).is_err());
}

#[test]
fn event_allowlist_requires_grant_and_payload_permissions_and_adds_one_resync() {
    let mut m = manifest();
    m.events.push(PluginEventSubscription::ResyncRequired);
    let grant = PluginGrant::authorize(&m, &[]).unwrap();
    assert!(grant.allowed_events().next().is_none());
    let grant = PluginGrant::authorize(&m, &[PluginPermission::EventsSubscribe]).unwrap();
    assert_eq!(
        grant.allowed_events().collect::<Vec<_>>(),
        vec![PluginEventSubscription::ResyncRequired]
    );
    let grant = PluginGrant::authorize(&m, &m.permissions).unwrap();
    assert_eq!(
        grant.allowed_events().collect::<Vec<_>>(),
        vec![
            PluginEventSubscription::SubmissionJudged,
            PluginEventSubscription::ResyncRequired
        ]
    );
}
