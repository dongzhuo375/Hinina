//! @plugin-module
//! @layer infra/plugin_repository
//! @version 2
//! @stability stable
//! Read-only code repository and host-owned approvals. Loading never grants new privileges.
use crate::{
    plugin::registry::*,
    plugin::{api::*, host::manifest::*, permission::PluginGrant},
};
use std::{
    collections::HashSet,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

/// @plugin-api
/// @since 2.0
/// @stability stable
pub struct FsPluginRepository {
    base: PathBuf,
}
impl FsPluginRepository {
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    pub fn new(base: PathBuf) -> Self {
        Self { base }
    }
}
impl PluginRepository for FsPluginRepository {
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    fn configuration(&self) -> PluginResult<PluginConfiguration> {
        let path = self.base.join("plugins.json");
        let config: PluginConfiguration = match read(&path, 65536) {
            Err(PluginError::NotFound) => return Ok(PluginConfiguration::default()),
            result => serde_json::from_slice(&result?).map_err(|_| PluginError::InvalidInput)?,
        };
        if config.plugins.len() > 128 || config.code_root.as_ref().is_some_and(|p| !p.is_absolute())
        {
            return Err(PluginError::InvalidInput);
        }
        for approval in config.plugins.values() {
            if !(10..=3600).contains(&approval.resync_interval_secs)
                || approval.options.to_string().len() > 65536
                || approval.network.len() > 32
            {
                return Err(PluginError::LimitExceeded);
            }
            for network in &approval.network {
                network.validate()?;
            }
        }
        Ok(config)
    }
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    fn load(&self, id: &str) -> PluginResult<LoadedPlugin> {
        let config = self.configuration()?;
        let approval = config
            .plugins
            .get(id)
            .filter(|p| p.enabled)
            .cloned()
            .ok_or(PluginError::Unavailable)?;
        let root = config
            .code_root
            .unwrap_or_else(|| self.base.join("plugins"));
        let root = root.canonicalize().map_err(|_| PluginError::Unavailable)?;
        let mut ids = HashSet::new();
        let mut selected = None;
        let entries = std::fs::read_dir(&root).map_err(|_| PluginError::Unavailable)?;
        for (index, entry) in entries.enumerate() {
            if index >= 128 {
                return Err(PluginError::LimitExceeded);
            }
            let entry = entry.map_err(|_| PluginError::Unavailable)?;
            if !entry
                .file_type()
                .map_err(|_| PluginError::Unavailable)?
                .is_dir()
            {
                continue;
            }
            let directory = entry
                .path()
                .canonicalize()
                .map_err(|_| PluginError::Unavailable)?;
            if !directory.starts_with(&root) {
                return Err(PluginError::InvalidInput);
            }
            let manifest_path = match directory.join("manifest.json").canonicalize() {
                Ok(path) if path.starts_with(&directory) => path,
                _ => continue,
            };
            // A malformed unrelated plugin must not prevent a valid plugin from starting.
            let raw: serde_json::Value = match read(&manifest_path, 65536).and_then(|bytes| {
                serde_json::from_slice(&bytes).map_err(|_| PluginError::InvalidInput)
            }) {
                Ok(raw) => raw,
                Err(_) => continue,
            };
            let Some(candidate) = raw["id"].as_str() else {
                continue;
            };
            if !ids.insert(candidate.to_string()) && candidate == id {
                return Err(PluginError::InvalidInput);
            }
            if candidate == id {
                if raw["manifestVersion"] != 2 || raw["apiVersion"] != 2 {
                    return Err(PluginError::Unsupported);
                }
                let manifest: PluginManifest =
                    serde_json::from_value(raw).map_err(|_| PluginError::InvalidInput)?;
                manifest.validate().map_err(|_| PluginError::InvalidInput)?;
                selected = Some((directory, manifest));
            }
        }
        let (directory, manifest) = selected.ok_or(PluginError::NotFound)?;
        if manifest.min_app_version.as_ref().is_some_and(|v| {
            semver::Version::parse(v).unwrap()
                > semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap()
        }) {
            return Err(PluginError::Unsupported);
        }
        let entry = directory
            .join(&manifest.entry)
            .canonicalize()
            .map_err(|_| PluginError::NotFound)?;
        if !entry.starts_with(&directory) {
            return Err(PluginError::InvalidInput);
        }
        // Intersection: removing a declaration cannot grant a capability on reload.
        let approved = approval
            .permissions
            .iter()
            .copied()
            .filter(|p| manifest.permissions.contains(p))
            .collect::<Vec<_>>();
        let grant = PluginGrant::authorize(&manifest, &approved)?;
        let source =
            String::from_utf8(read(&entry, 1024 * 1024)?).map_err(|_| PluginError::InvalidInput)?;
        Ok(LoadedPlugin {
            grant,
            source,
            approval,
        })
    }
}
fn read(path: &Path, limit: usize) -> PluginResult<Vec<u8>> {
    let file = File::open(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            PluginError::NotFound
        } else {
            PluginError::Unavailable
        }
    })?;
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| PluginError::Unavailable)?;
    if bytes.len() > limit {
        return Err(PluginError::LimitExceeded);
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "tests/fs_plugin_repo_tests.rs"]
mod tests;
