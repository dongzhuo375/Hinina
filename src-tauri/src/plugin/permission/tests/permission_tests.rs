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
