use super::*;
use crate::plugin::test_support::authority;

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
async fn abandoned_write_is_not_committed_after_the_caller_times_out() {
    let dir = crate::test_support::TempDir::unique("plugin-json-abandon");
    let storage = Arc::new(JsonPluginStorage::new(
        dir.to_path_buf(),
        authority("test.abandon"),
    ));
    storage.set("seed", Value::Bool(true)).await.unwrap();
    std::fs::create_dir_all(&storage.directory).unwrap();
    // Hold the storage lock so the write is still queued when the caller is dropped.
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(storage.directory.join("storage.lock"))
        .unwrap();
    FileExt::try_lock(&lock).unwrap();
    let writer = {
        let storage = storage.clone();
        tokio::spawn(async move { storage.set("late", Value::Bool(true)).await })
    };
    while storage.concurrent.available_permits() == 16 {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    writer.abort();
    let _ = writer.await;
    FileExt::unlock(&lock).unwrap();
    // The detached blocking task resumes, observes the abandonment and must not commit.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        storage.get("late").await.unwrap(),
        PluginStorageValue::Missing
    );
    assert_eq!(
        storage.get("seed").await.unwrap(),
        PluginStorageValue::Value {
            value: Value::Bool(true)
        }
    );
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

#[tokio::test]
async fn storage_operation_budget_rejects_reads_writes_deletes_and_cas_before_io() {
    let dir = crate::test_support::TempDir::unique("plugin-storage-budget");
    let storage = JsonPluginStorage::new(dir.to_path_buf(), authority("test.budget"));
    let shared = Arc::new(RateLimiter::direct(Quota::per_minute(
        NonZeroU32::new(1).unwrap(),
    )));
    let storage = storage.with_shared_budget(shared);
    storage.set("key", Value::Bool(true)).await.unwrap();
    let original = std::fs::read(storage.directory.join("storage.json")).unwrap();
    assert_eq!(
        storage.set("key", Value::Null).await,
        Err(PluginError::LimitExceeded)
    );
    assert_eq!(storage.delete("key").await, Err(PluginError::LimitExceeded));
    assert_eq!(storage.get("key").await, Err(PluginError::LimitExceeded));
    assert_eq!(
        storage
            .compare_and_set("key", PluginStorageValue::Missing, Value::Null)
            .await,
        Err(PluginError::LimitExceeded)
    );
    assert_eq!(
        std::fs::read(storage.directory.join("storage.json")).unwrap(),
        original
    );
}
