use super::*;
use crate::plugin::test_support::manifest;
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
fn many_or_broken_sibling_entries_do_not_block_a_valid_plugin() {
    let dir = crate::test_support::TempDir::unique("plugin-repo-siblings");
    let repo = setup(&dir);
    // A plain file and a directory without a manifest are unrelated entries.
    std::fs::write(dir.join("plugins/notes.txt"), "ignored").unwrap();
    std::fs::create_dir_all(dir.join("plugins/zz-empty")).unwrap();
    // Sibling directories below the scan bound: the valid plugin is found regardless of the
    // unspecified read_dir order, and the broken/empty siblings never block it.
    for index in 0..100 {
        std::fs::create_dir_all(dir.join(format!("plugins/zz-noise-{index:03}"))).unwrap();
    }
    let loaded = repo.load("test.repo").unwrap();
    assert_eq!(loaded.grant.manifest().id, "test.repo");
}

#[test]
fn directory_bound_reports_truncation_instead_of_false_absence() {
    let dir = crate::test_support::TempDir::unique("plugin-repo-dir-bound");
    let repo = setup(&dir);
    // Approve a target that has no directory so `load` reaches the scan, then surround it
    // with more directory candidates than MAX_PLUGIN_DIRECTORIES.
    std::fs::write(
        dir.join("plugins.json"),
        serde_json::json!({"plugins":{
            "test.repo":{"enabled":true,"permissions":["events.subscribe"]},
            "test.absent":{"enabled":true,"permissions":["events.subscribe"]}
        }})
        .to_string(),
    )
    .unwrap();
    for index in 0..(MAX_PLUGIN_DIRECTORIES + 8) {
        std::fs::create_dir_all(dir.join(format!("plugins/zz-noise-{index:04}"))).unwrap();
    }
    // The scan hits the directory bound without finding the target: report truncation
    // (LimitExceeded), never NotFound. Order-independent because the target has no directory.
    assert!(matches!(
        repo.load("test.absent"),
        Err(crate::plugin::api::PluginError::LimitExceeded)
    ));
}

#[test]
fn entry_bound_caps_a_code_root_full_of_plain_files() {
    let dir = crate::test_support::TempDir::unique("plugin-repo-entry-bound");
    let repo = setup(&dir);
    std::fs::write(
        dir.join("plugins.json"),
        serde_json::json!({"plugins":{
            "test.repo":{"enabled":true,"permissions":["events.subscribe"]},
            "test.absent":{"enabled":true,"permissions":["events.subscribe"]}
        }})
        .to_string(),
    )
    .unwrap();
    // Plain files never count toward the directory bound, so without a total-entry cap a
    // code root full of loose files would be iterated without limit on every load. The
    // entry bound truncates the scan; the absent target reports LimitExceeded, not NotFound.
    for index in 0..(MAX_SCAN_ENTRIES + 8) {
        std::fs::write(dir.join(format!("plugins/file-{index:05}.txt")), "x").unwrap();
    }
    assert!(matches!(
        repo.load("test.absent"),
        Err(crate::plugin::api::PluginError::LimitExceeded)
    ));
}

#[cfg(unix)]
#[test]
fn a_symlink_escaping_the_code_root_is_skipped_not_fatal() {
    let dir = crate::test_support::TempDir::unique("plugin-repo-symlink");
    let repo = setup(&dir);
    let outside = crate::test_support::TempDir::unique("plugin-repo-outside");
    std::fs::create_dir_all(outside.join("plugin")).unwrap();
    std::fs::write(
        outside.join("plugin/manifest.json"),
        serde_json::to_vec(&manifest("test.outside")).unwrap(),
    )
    .unwrap();
    std::os::unix::fs::symlink(outside.join("plugin"), dir.join("plugins/escape")).unwrap();
    // The escaping link is ignored; the in-root plugin still loads.
    let loaded = repo.load("test.repo").unwrap();
    assert_eq!(loaded.grant.manifest().id, "test.repo");
    assert!(repo.load("test.outside").is_err());
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
