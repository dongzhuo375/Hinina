use super::*;
use crate::plugin::tests::support::manifest;
fn setup(dir: &crate::test_support::TempDir) -> FsPluginRepository {
    let code = dir.join("plugins/test.repo");
    std::fs::create_dir_all(&code).unwrap();
    std::fs::write(
        code.join("manifest.json"),
        serde_json::to_vec(&manifest("test.repo")).unwrap(),
    )
    .unwrap();
    std::fs::write(code.join("index.js"), "export default async () => {};").unwrap();
    std::fs::write(dir.join("plugins.json"),serde_json::json!({"plugins":{"test.repo":{"enabled":true,"permissions":["events.subscribe"]}}}).to_string()).unwrap();
    FsPluginRepository::new(dir.to_path_buf())
}
#[test]
fn approvals_are_outside_code_and_do_not_expand_with_manifest() {
    let dir = crate::test_support::TempDir::unique("plugin-repo");
    let repo = setup(&dir);
    let loaded = repo.load("test.repo").unwrap();
    assert!(loaded
        .grant
        .require(PluginPermission::EventsSubscribe)
        .is_ok());
    assert!(loaded
        .grant
        .require(PluginPermission::NetworkRequest)
        .is_err());
    assert_eq!(loaded.approval.resync_interval_secs, 60);
    assert!(repo.load("../test.repo").is_err());
    std::fs::remove_file(dir.join("plugins.json")).unwrap();
    assert!(repo.configuration().unwrap().plugins.is_empty());
    assert!(repo.load("test.repo").is_err());
}
#[test]
fn duplicate_ids_missing_entry_and_old_manifest_are_rejected() {
    let dir = crate::test_support::TempDir::unique("plugin-repo-bad");
    let repo = setup(&dir);
    std::fs::create_dir_all(dir.join("plugins/duplicate")).unwrap();
    std::fs::copy(
        dir.join("plugins/test.repo/manifest.json"),
        dir.join("plugins/duplicate/manifest.json"),
    )
    .unwrap();
    assert!(repo.load("test.repo").is_err());
    std::fs::remove_file(dir.join("plugins/duplicate/manifest.json")).unwrap();
    std::fs::remove_dir(dir.join("plugins/duplicate")).unwrap();
    std::fs::remove_file(dir.join("plugins/test.repo/index.js")).unwrap();
    assert!(repo.load("test.repo").is_err());
    let mut m = manifest("test.repo");
    m.manifest_version = 1;
    std::fs::write(
        dir.join("plugins/test.repo/manifest.json"),
        serde_json::to_vec(&m).unwrap(),
    )
    .unwrap();
    assert!(repo.load("test.repo").is_err());
}

#[test]
fn invalid_unrelated_approvals_do_not_block_loading_a_valid_plugin() {
    let dir = crate::test_support::TempDir::unique("plugin-approval-isolation");
    let repo = setup(&dir);
    std::fs::write(
        dir.join("plugins.json"),
        serde_json::json!({"plugins": {
            "test.repo": {"enabled": true, "permissions": ["events.subscribe"]},
            "test.bad": {"enabled": true, "resyncIntervalSecs": 1},
            "test.malformed": {"permissions": ["not-a-permission"]}
        }})
        .to_string(),
    )
    .unwrap();
    let config = repo.configuration().unwrap();
    assert_eq!(config.plugins.len(), 1);
    assert_eq!(config.invalid_plugins.len(), 2);
    repo.load("test.repo").unwrap();
    assert!(matches!(
        repo.load("test.bad"),
        Err(PluginError::LimitExceeded)
    ));
    assert!(matches!(
        repo.load("test.malformed"),
        Err(PluginError::InvalidInput)
    ));
}
