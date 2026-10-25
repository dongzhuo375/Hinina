use super::*;

fn manifest() -> PluginManifest {
    PluginManifest {
        id: "com.example.demo".into(),
        name: "Demo".into(),
        version: "1.2.3-beta.1+build.4".into(),
        manifest_version: 2,
        runtime: "js".into(),
        entry: "src/index.js".into(),
        api_version: 2,
        description: String::new(),
        author: String::new(),
        min_app_version: Some("0.1.0".into()),
        permissions: vec![PluginPermission::EventsSubscribe],
        events: vec![PluginEventSubscription::SubmissionJudged],
        network: Default::default(),
        contributes: vec![],
        config_schema: serde_json::json!({}),
    }
}

#[test]
fn valid_manifest_is_accepted_and_uses_stable_wire_names() {
    let value = manifest();
    value.validate().unwrap();
    let json = serde_json::to_value(value).unwrap();
    assert_eq!(json["manifestVersion"], 2);
    assert_eq!(json["apiVersion"], 2);
    assert_eq!(json["events"][0], "submission.judged");
    assert_eq!(json["permissions"][0], "events.subscribe");
}

#[test]
fn contest_rank_permission_parses_and_typos_are_rejected() {
    assert_eq!(
        serde_json::to_value(PluginPermission::ContestRank).unwrap(),
        "contest.rank"
    );
    assert_eq!(
        serde_json::from_value::<PluginPermission>("contest.rank".into()).unwrap(),
        PluginPermission::ContestRank
    );
    // Misspellings must fail loudly instead of silently becoming no-ops.
    for typo in ["contest.ranking", "contestRank", "contest.ranks"] {
        assert!(
            serde_json::from_value::<PluginPermission>(typo.into()).is_err(),
            "{typo} must not parse"
        );
    }
}

#[test]
fn approving_an_undeclared_contest_rank_is_rejected() {
    let mut value = manifest();
    value.permissions = vec![PluginPermission::ContestRead];
    value.events = vec![];
    value.validate().unwrap();
    // Approval is an intersection; approving an undeclared permission is a
    // configuration error, not a silent no-op.
    let error =
        crate::plugin::permission::PluginGrant::authorize(&value, &[PluginPermission::ContestRank])
            .unwrap_err();
    assert_eq!(error, crate::plugin::api::PluginError::InvalidInput);
    // Declared and approved keeps working.
    crate::plugin::permission::PluginGrant::authorize(&value, &[PluginPermission::ContestRead])
        .unwrap();
}

#[test]
fn invalid_identity_versions_runtime_and_entry_are_rejected() {
    let mut value = manifest();
    value.id = "../escape".into();
    assert_eq!(value.validate(), Err(ManifestError("id")));
    let mut value = manifest();
    value.version = "01.2.3".into();
    assert_eq!(value.validate(), Err(ManifestError("version")));
    let mut value = manifest();
    value.runtime = "wasm".into();
    assert_eq!(value.validate(), Err(ManifestError("runtime")));
    for entry in [
        "../index.js",
        "C:/plugin/index.js",
        "src\\index.js",
        "/index.js",
        "index.ts",
    ] {
        let mut value = manifest();
        value.entry = entry.into();
        assert_eq!(value.validate(), Err(ManifestError("entry")), "{entry}");
    }
}

#[test]
fn empty_events_do_not_require_event_subscription_permission() {
    let mut value = manifest();
    value.permissions.clear();
    value.events.clear();
    value.validate().unwrap();
}

#[test]
fn network_policy_requires_permission_and_normalized_domain_origin() {
    let mut value = manifest();
    value.permissions.push(PluginPermission::NetworkRequest);
    value.network.allow_origins = vec!["https://balloon.example.com".into()];
    value.validate().unwrap();

    for host in [
        "localhost",
        "127.0.0.1",
        "10.0.0.1",
        "x.local",
        "http://127.0.0.1",
        "https://169.254.169.254",
        "https://8.8.8.8",
        "https://[2001:4860:4860::8888]",
        "http://[::1]",
        "https://[fe80::1]",
        "http://2130706433",
        "https://*.example.com",
        "https://example.com/path",
    ] {
        let mut value = manifest();
        value.permissions.push(PluginPermission::NetworkRequest);
        value.network.allow_origins = vec![host.into()];
        assert_eq!(value.validate(), Err(ManifestError("network")), "{host}");
    }
    let mut hostname = manifest();
    hostname.permissions.push(PluginPermission::NetworkRequest);
    hostname.network.allow_origins = vec!["https://public.example.com".into()];
    hostname.validate().unwrap();
    hostname.network.allow_origins = vec!["http://balloons.internal:8080".into()];
    hostname.validate().unwrap(); // Runtime DNS/IP policy still validates all resolved addresses.
    let mut value = manifest();
    value.network.allow_origins = vec!["https://balloon.example.com".into()];
    assert_eq!(value.validate(), Err(ManifestError("network")));
}

#[test]
fn duplicate_permissions_events_and_unimplemented_extensions_are_rejected() {
    let mut value = manifest();
    value.permissions.push(PluginPermission::EventsSubscribe);
    assert_eq!(value.validate(), Err(ManifestError("permissions")));
    let mut value = manifest();
    value.events.push(PluginEventSubscription::SubmissionJudged);
    assert_eq!(value.validate(), Err(ManifestError("events")));
    let mut value = manifest();
    value.permissions.clear();
    assert_eq!(value.validate(), Err(ManifestError("events")));
    let mut value = manifest();
    value
        .contributes
        .push(serde_json::json!({"type":"sidebar"}));
    assert_eq!(value.validate(), Err(ManifestError("contributes")));
    let mut value = manifest();
    value.config_schema = serde_json::json!({"type":"object"});
    assert_eq!(value.validate(), Err(ManifestError("configSchema")));
}

#[test]
fn unknown_wire_permissions_events_and_fields_are_rejected() {
    for json in [
        r#"{"id":"x","name":"x","version":"1.0.0","manifestVersion":2,"runtime":"js","entry":"index.js","apiVersion":2,"permissions":["filesystem.read"],"events":[]}"#,
        r#"{"id":"x","name":"x","version":"1.0.0","manifestVersion":2,"runtime":"js","entry":"index.js","apiVersion":2,"permissions":[],"events":["submission.deleted"]}"#,
        r#"{"id":"x","name":"x","version":"1.0.0","manifestVersion":2,"runtime":"js","entry":"index.js","apiVersion":2,"permissions":[],"events":[],"runtimeVersion":1}"#,
    ] {
        assert!(serde_json::from_str::<PluginManifest>(json).is_err());
    }
}

#[test]
fn versions_missing_entry_and_network_limits_are_checked() {
    for version in ["1.2", "01.2.3", "1.2.3-01", "1.2.3+", "1.2.3-a..b"] {
        let mut value = manifest();
        value.version = version.into();
        assert_eq!(value.validate(), Err(ManifestError("version")));
    }
    let mut value = manifest();
    value.manifest_version = 1;
    assert_eq!(value.validate(), Err(ManifestError("manifestVersion")));
    value.manifest_version = 2;
    value.api_version = 1;
    assert_eq!(value.validate(), Err(ManifestError("apiVersion")));
    let mut json = serde_json::to_value(manifest()).unwrap();
    json.as_object_mut().unwrap().remove("entry");
    assert!(serde_json::from_value::<PluginManifest>(json).is_err());
    for (bytes, timeout) in [(65_537, 1), (1, 5_001)] {
        let mut value = manifest();
        value.network.max_body_bytes = bytes;
        value.network.timeout_ms = timeout;
        assert_eq!(value.validate(), Err(ManifestError("network")));
    }
}

#[test]
fn documented_manifest_matches_the_implemented_contract() {
    let example = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../doc/modules/src-tauri/plugin/sdk/manifest.example.json"
    ));
    let manifest: PluginManifest = serde_json::from_str(example).unwrap();
    manifest.validate().unwrap();
    assert_eq!(manifest.id, "com.example.development");
}
