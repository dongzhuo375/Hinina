use super::*;
use crate::test_support::{Guarded, TempDir};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TestConfig {
    name: String,
    theme: String,
    timeout: u64,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            name: "Hinina".into(),
            theme: "dark".into(),
            timeout: 30,
        }
    }
}

/// 测试仓库 + 临时目录守卫（`Drop` 时删除目录，避免临时文件堆积）。
fn repo(name: &str) -> Guarded<FsConfigRepository> {
    let dir = TempDir::named(&format!("hinina-test-config-{}", name));
    let repo = FsConfigRepository::new(Arc::new(Storage::new(dir.to_path_buf())), "config.json");
    Guarded::new(repo, dir)
}

#[test]
fn save_and_load_roundtrip() {
    let r = repo("roundtrip");
    let cfg = TestConfig::default();

    r.save_config(&cfg).unwrap();
    assert!(r.config_exists());

    let loaded: TestConfig = r.load_config().unwrap();
    assert_eq!(loaded, cfg);
}

#[test]
fn config_exists_returns_false_initially() {
    let r = repo("exists-false");
    assert!(!r.config_exists());
}

#[test]
fn load_missing_config_returns_error() {
    let r = repo("missing");
    let result: AppResult<TestConfig> = r.load_config();
    assert!(result.is_err());
}

#[test]
fn load_invalid_json_returns_error() {
    let r = repo("invalid-json");
    // 手动写入非法 JSON
    r.storage.write_string("config.json", "not valid json").unwrap();
    let result: AppResult<TestConfig> = r.load_config();
    assert!(result.is_err());
}

#[test]
fn save_then_exists() {
    let r = repo("save-exists");
    assert!(!r.config_exists());
    r.save_config(&TestConfig::default()).unwrap();
    assert!(r.config_exists());
}
