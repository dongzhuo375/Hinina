//! @plugin-module
//! @layer infra/plugin_repository
//! @version 2
//! @stability stable
//! Read-only code repository and host-owned approvals. Loading never grants new privileges.
use super::plugin_diagnostics as diagnostics;
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

/// Upper bound on directory candidates inspected per load. Exceeding it only limits the
/// lookup of a plugin that was not found, never an already valid sibling.
const MAX_PLUGIN_DIRECTORIES: usize = 128;
/// Upper bound on total `read_dir` entries consumed per load, including non-directories.
/// Directory symlinks inside the code root are followed to their canonical target (which
/// must stay within the root); plain files are skipped but still counted here, so a code
/// root holding many loose files cannot make `load` an unbounded scan. Like the directory
/// bound, exceeding it only affects a not-found lookup, which reports `LimitExceeded`.
const MAX_SCAN_ENTRIES: usize = 1024;
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
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct RawConfiguration {
            code_root: Option<PathBuf>,
            #[serde(default)]
            plugins: std::collections::BTreeMap<String, serde_json::Value>,
        }
        let raw: RawConfiguration = match read(&path, 65536, None, "repository.configuration") {
            Err(PluginError::NotFound) => return Ok(PluginConfiguration::default()),
            result => serde_json::from_slice(&result?).map_err(|error| {
                diagnostics::json(None, "repository.configuration_decode", &error);
                PluginError::InvalidInput
            })?,
        };
        if raw.plugins.len() > 128 || raw.code_root.as_ref().is_some_and(|p| !p.is_absolute()) {
            diagnostics::failure(
                None,
                "repository.configuration_validate",
                "invalid_configuration",
            );
            return Err(PluginError::InvalidInput);
        }
        let mut config = PluginConfiguration {
            code_root: raw.code_root,
            ..Default::default()
        };
        for (id, value) in raw.plugins {
            let approval = serde_json::from_value::<PluginApproval>(value)
                .map_err(|_| PluginError::InvalidInput)
                .and_then(|approval| {
                    approval.validate()?;
                    Ok(approval)
                });
            match approval {
                Ok(approval) => {
                    config.plugins.insert(id, approval);
                }
                Err(error) => {
                    diagnostics::failure(Some(&id), "repository.approval", "invalid_approval");
                    config.invalid_plugins.insert(id, error);
                }
            }
        }
        Ok(config)
    }
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    fn load(&self, id: &str) -> PluginResult<LoadedPlugin> {
        let unavailable = |operation, error: std::io::Error| {
            diagnostics::io(Some(id), operation, &error);
            PluginError::Unavailable
        };
        let config = self.configuration()?;
        if let Some(error) = config.invalid_plugins.get(id) {
            return Err(error.clone());
        }
        let approval = config
            .plugins
            .get(id)
            .filter(|p| p.enabled)
            .cloned()
            .ok_or(PluginError::Unavailable)?;
        let root = config
            .code_root
            .unwrap_or_else(|| self.base.join("plugins"));
        let root = root
            .canonicalize()
            .map_err(|error| unavailable("repository.root", error))?;
        let mut ids = HashSet::new();
        let mut selected = None;
        let entries =
            std::fs::read_dir(&root).map_err(|error| unavailable("repository.scan", error))?;
        let mut scanned = 0;
        let mut truncated = false;
        for (index, entry) in entries.enumerate() {
            // Bound both the directory candidates and the total entries consumed, so a code
            // root with many plain files cannot make `load` an unbounded scan.
            if scanned >= MAX_PLUGIN_DIRECTORIES || index >= MAX_SCAN_ENTRIES {
                truncated = true;
                break;
            }
            // Every per-entry problem is local to that entry: a broken link, an escaping
            // link or a malformed manifest must never block loading a different plugin.
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    diagnostics::io(None, "repository.entry", &error);
                    continue;
                }
            };
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(error) => {
                    diagnostics::io(None, "repository.entry_type", &error);
                    continue;
                }
            };
            // Directory symlinks are followed only to their canonical target, which must
            // stay within the root (checked below); escaping links are skipped, not fatal.
            if !(file_type.is_dir() || (file_type.is_symlink() && path.is_dir())) {
                continue;
            }
            scanned += 1;
            let directory = match path.canonicalize() {
                Ok(directory) => directory,
                Err(error) => {
                    diagnostics::io(None, "repository.directory", &error);
                    continue;
                }
            };
            if !directory.starts_with(&root) {
                diagnostics::failure(None, "repository.directory", "path_boundary");
                continue;
            }
            let manifest_path = match directory.join("manifest.json").canonicalize() {
                Ok(path) if path.starts_with(&directory) => path,
                Ok(_) => {
                    diagnostics::failure(None, "repository.manifest", "path_boundary");
                    continue;
                }
                Err(error) => {
                    if error.kind() != std::io::ErrorKind::NotFound {
                        diagnostics::io(None, "repository.manifest", &error);
                    }
                    continue;
                }
            };
            // A malformed unrelated plugin must not prevent a valid plugin from starting.
            let raw: serde_json::Value =
                match read(&manifest_path, 65536, None, "repository.manifest_read").and_then(
                    |bytes| {
                        serde_json::from_slice(&bytes).map_err(|error| {
                            diagnostics::json(None, "repository.manifest_decode", &error);
                            PluginError::InvalidInput
                        })
                    },
                ) {
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
                let manifest: PluginManifest = serde_json::from_value(raw).map_err(|error| {
                    diagnostics::json(Some(id), "repository.manifest_type", &error);
                    PluginError::InvalidInput
                })?;
                manifest.validate().map_err(|_| {
                    diagnostics::failure(
                        Some(id),
                        "repository.manifest_validate",
                        "invalid_manifest",
                    );
                    PluginError::InvalidInput
                })?;
                selected = Some((directory, manifest));
            }
        }
        let Some((directory, manifest)) = selected else {
            // The scan bound was reached without finding the target: report the truncation
            // instead of pretending the plugin does not exist.
            return Err(if truncated {
                PluginError::LimitExceeded
            } else {
                PluginError::NotFound
            });
        };
        if manifest.min_app_version.as_ref().is_some_and(|v| {
            semver::Version::parse(v).unwrap()
                > semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap()
        }) {
            return Err(PluginError::Unsupported);
        }
        let entry = directory
            .join(&manifest.entry)
            .canonicalize()
            .map_err(|error| {
                diagnostics::io(Some(id), "repository.source_path", &error);
                PluginError::NotFound
            })?;
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
        let source = String::from_utf8(read(
            &entry,
            1024 * 1024,
            Some(id),
            "repository.source_read",
        )?)
        .map_err(|_| {
            diagnostics::failure(Some(id), "repository.source_decode", "invalid_utf8");
            PluginError::InvalidInput
        })?;
        Ok(LoadedPlugin {
            grant,
            source,
            approval,
        })
    }
}
fn read(
    path: &Path,
    limit: usize,
    id: Option<&str>,
    operation: &'static str,
) -> PluginResult<Vec<u8>> {
    let file = File::open(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            PluginError::NotFound
        } else {
            diagnostics::io(id, operation, &e);
            PluginError::Unavailable
        }
    })?;
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            diagnostics::io(id, operation, &error);
            PluginError::Unavailable
        })?;
    if bytes.len() > limit {
        diagnostics::failure(id, operation, "file_size_limit");
        return Err(PluginError::LimitExceeded);
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "tests/fs_plugin_repo_tests.rs"]
mod tests;
