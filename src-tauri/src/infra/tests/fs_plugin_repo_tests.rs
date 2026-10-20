use super::*;

#[test]
fn placeholder_reports_unsupported_instead_of_panicking_or_claiming_empty() {
    let repo = FsPluginRepository::new();
    assert!(matches!(repo.scan_plugins(), Err(PluginError::Unsupported)));
    assert_eq!(repo.plugin_dir_exists(), Err(PluginError::Unsupported));
    let manifest = serde_json::from_value(serde_json::json!({
        "id":"com.example.demo","name":"Demo","version":"1.0.0",
        "manifestVersion":1,"apiVersion":1,"runtime":"js","entry":"index.js",
        "permissions":[],"events":[]
    }))
    .unwrap();
    assert_eq!(
        repo.load_plugin_code(&manifest),
        Err(PluginError::Unsupported)
    );
}
