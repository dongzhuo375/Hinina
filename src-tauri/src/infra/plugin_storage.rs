//! @plugin-module
//! @layer infra/plugin_storage
//! @version 2
//! @stability stable
//! Per-plugin JSON, separate lock inode and durable atomic replacement. Corruption is never reset.
use crate::plugin::{
    api::{storage::*, *},
    host::manifest::PluginPermission,
    query::ScopedQueries,
};
use atomicwrites::{AllowOverwrite, AtomicFile};
use fs4::FileExt;
use governor::{DefaultDirectRateLimiter, Quota, RateLimiter};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{Read, Write},
    num::NonZeroU32,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

/// @plugin-api
/// @since 2.0
/// @permission storage.read / storage.write
/// @stability stable
/// Hard quota includes JSON keys and encoding overhead.
pub const MAX_STORAGE_BYTES: usize = 1024 * 1024;

/// @plugin-api
/// @since 2.0
/// @stability stable
/// No API accepts a path; the namespace comes only from a host grant.
pub struct JsonPluginStorage {
    directory: PathBuf,
    authority: Arc<ScopedQueries>,
    operations: DefaultDirectRateLimiter,
    shared: Option<Arc<DefaultDirectRateLimiter>>,
    concurrent: Arc<tokio::sync::Semaphore>,
}
impl JsonPluginStorage {
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    pub fn new(data_root: PathBuf, authority: Arc<ScopedQueries>) -> Self {
        let namespace = format!(
            "{:x}",
            Sha256::digest(authority.grant.manifest().id.as_bytes())
        );
        Self {
            directory: data_root.join(namespace),
            authority,
            operations: RateLimiter::direct(Quota::per_minute(NonZeroU32::new(60).unwrap())),
            shared: None,
            concurrent: Arc::new(tokio::sync::Semaphore::new(16)),
        }
    }
    pub fn with_shared_budget(mut self, budget: Arc<DefaultDirectRateLimiter>) -> Self {
        self.shared = Some(budget);
        self
    }
    async fn transaction<T: Send + 'static>(
        &self,
        edit: impl FnOnce(&mut BTreeMap<String, Value>) -> PluginResult<(T, bool)> + Send + 'static,
    ) -> PluginResult<T> {
        let permit = self
            .concurrent
            .clone()
            .try_acquire_owned()
            .map_err(|_| PluginError::LimitExceeded)?;
        self.operations
            .check()
            .map_err(|_| PluginError::LimitExceeded)?;
        if let Some(shared) = &self.shared {
            shared.check().map_err(|_| PluginError::LimitExceeded)?;
        }
        let directory = self.directory.clone();
        let stopped = self.authority.stopped.clone();
        tokio::task::spawn_blocking(move || {
            // Keep capacity reserved even if the waiting JS future is dropped.
            let _permit = permit;
            let unavailable = |_| PluginError::Unavailable;
            std::fs::create_dir_all(&directory).map_err(unavailable)?;
            let lock = OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(directory.join("storage.lock"))
                .map_err(unavailable)?;
            let start = Instant::now();
            loop {
                if stopped.is_cancelled() {
                    return Err(PluginError::ContextExpired);
                }
                match FileExt::try_lock(&lock) {
                    Ok(()) => break,
                    Err(fs4::TryLockError::WouldBlock) => {
                        if start.elapsed() > Duration::from_secs(5) {
                            return Err(PluginError::Timeout);
                        }
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => return Err(PluginError::Unavailable),
                }
            }
            let path = directory.join("storage.json");
            let mut data = match File::open(&path) {
                Ok(file) => {
                    let mut bytes = Vec::new();
                    file.take((MAX_STORAGE_BYTES + 1) as u64)
                        .read_to_end(&mut bytes)
                        .map_err(unavailable)?;
                    if bytes.len() > MAX_STORAGE_BYTES {
                        return Err(PluginError::LimitExceeded);
                    }
                    serde_json::from_slice::<BTreeMap<String, Value>>(&bytes)
                        .map_err(|_| PluginError::Unavailable)?
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
                Err(_) => return Err(PluginError::Unavailable),
            };
            if stopped.is_cancelled() {
                return Err(PluginError::ContextExpired);
            }
            let (result, changed) = edit(&mut data)?;
            if changed {
                let bytes = serde_json::to_vec(&data).map_err(|_| PluginError::InvalidInput)?;
                if bytes.len() > MAX_STORAGE_BYTES {
                    return Err(PluginError::LimitExceeded);
                }
                AtomicFile::new(&path, AllowOverwrite)
                    .write(|file| file.write_all(&bytes))
                    .map_err(|_| PluginError::Unavailable)?;
            }
            Ok(result)
        })
        .await
        .map_err(|_| PluginError::Unavailable)?
    }
    /// @plugin-api
    /// @since 2.0
    /// @permission storage.read
    /// @stability stable
    pub async fn get(&self, key: &str) -> PluginResult<PluginStorageValue> {
        self.authority.require(PluginPermission::StorageRead)?;
        validate_key(key)?;
        let key = key.to_string();
        self.transaction(move |data| Ok((stored(data.get(&key)), false)))
            .await
    }
    /// @plugin-api
    /// @since 2.0
    /// @permission storage.write
    /// @stability stable
    pub async fn set(&self, key: &str, value: Value) -> PluginResult<()> {
        self.authority.require(PluginPermission::StorageWrite)?;
        validate_key(key)?;
        validate_value(&value)?;
        let key = key.to_string();
        self.transaction(move |data| {
            data.insert(key, value);
            Ok(((), true))
        })
        .await
    }
    /// @plugin-api
    /// @since 2.0
    /// @permission storage.write
    /// @stability stable
    pub async fn delete(&self, key: &str) -> PluginResult<()> {
        self.authority.require(PluginPermission::StorageWrite)?;
        validate_key(key)?;
        let key = key.to_string();
        self.transaction(move |data| {
            let changed = data.remove(&key).is_some();
            Ok(((), changed))
        })
        .await
    }
    /// @plugin-api
    /// @since 2.0
    /// @permission storage.read / storage.write
    /// @stability stable
    pub async fn compare_and_set(
        &self,
        key: &str,
        expected: PluginStorageValue,
        value: Value,
    ) -> PluginResult<bool> {
        self.authority.require(PluginPermission::StorageRead)?;
        self.authority.require(PluginPermission::StorageWrite)?;
        validate_key(key)?;
        validate_value(&value)?;
        let key = key.to_string();
        self.transaction(move |data| {
            if stored(data.get(&key)) != expected {
                return Ok((false, false));
            }
            data.insert(key, value);
            Ok((true, true))
        })
        .await
    }
}
fn stored(value: Option<&Value>) -> PluginStorageValue {
    value.cloned().map_or(PluginStorageValue::Missing, |value| {
        PluginStorageValue::Value { value }
    })
}
#[cfg(test)]
#[path = "tests/plugin_storage_tests.rs"]
mod tests;
