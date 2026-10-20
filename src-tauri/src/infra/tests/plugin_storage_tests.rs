use super::*;
use crate::plugin::tests::support::authority;

#[tokio::test]
async fn private_json_cas_is_atomic_durable_and_distinguishes_null() {
    let dir = crate::test_support::TempDir::unique("plugin-json-cas");
    let a = Arc::new(JsonPluginStorage::new(
        dir.to_path_buf(),
        authority("test.a"),
    ));
    let b = JsonPluginStorage::new(dir.to_path_buf(), authority("test.b"));
    let (x, y) = tokio::join!(
        a.compare_and_set("../data", PluginStorageValue::Missing, Value::Null),
        a.compare_and_set("../data", PluginStorageValue::Missing, Value::Bool(true))
    );
    assert_ne!(x.unwrap(), y.unwrap());
    assert_eq!(b.get("../data").await.unwrap(), PluginStorageValue::Missing);
    let restarted = JsonPluginStorage::new(dir.to_path_buf(), authority("test.a"));
    assert_eq!(
        restarted.get("../data").await.unwrap(),
        a.get("../data").await.unwrap()
    );
    a.set("null", Value::Null).await.unwrap();
    assert!(!a
        .compare_and_set("null", PluginStorageValue::Missing, Value::Bool(true))
        .await
        .unwrap());
    assert!(a
        .set("big", Value::String("x".repeat(65536)))
        .await
        .is_err());
}
#[tokio::test]
async fn corruption_is_retained_and_permissions_are_enforced() {
    let dir = crate::test_support::TempDir::unique("plugin-json-corruption");
    let storage = JsonPluginStorage::new(dir.to_path_buf(), authority("test.a"));
    storage.set("key", Value::Null).await.unwrap();
    std::fs::write(storage.directory.join("storage.json"), b"broken").unwrap();
    assert_eq!(
        storage.set("key", Value::Null).await,
        Err(PluginError::Unavailable)
    );
    assert_eq!(
        std::fs::read(storage.directory.join("storage.json")).unwrap(),
        b"broken"
    );
}

#[tokio::test]
async fn total_quota_is_enforced_without_overwriting_existing_data() {
    let dir = crate::test_support::TempDir::unique("plugin-json-quota");
    let storage = JsonPluginStorage::new(dir.to_path_buf(), authority("test.quota"));
    let value = Value::String("x".repeat(65_530));
    for n in 0..15 {
        storage.set(&n.to_string(), value.clone()).await.unwrap();
    }
    assert_eq!(
        storage
            .set("overflow", Value::String("x".repeat(65_536)))
            .await,
        Err(PluginError::LimitExceeded)
    );
    assert_eq!(
        storage
            .compare_and_set("overflow", PluginStorageValue::Missing, value)
            .await,
        Err(PluginError::LimitExceeded)
    );
    assert_eq!(
        storage.get("overflow").await.unwrap(),
        PluginStorageValue::Missing
    );
}
