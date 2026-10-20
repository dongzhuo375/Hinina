//! @plugin-module
//! @layer plugin/registry
//! @version 2
//! @stability stable
//! Host-owned configuration and validated code port. Filesystem I/O belongs to Infra.
use crate::plugin::{
    api::{http::NetworkApproval, *},
    host::manifest::*,
    permission::PluginGrant,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

/// @plugin-api
/// @since 2.0
/// @permission host-only
/// @stability stable
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginApproval {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub permissions: Vec<PluginPermission>,
    #[serde(default)]
    pub network: Vec<NetworkApproval>,
    #[serde(default = "interval")]
    pub resync_interval_secs: u64,
    #[serde(default = "options")]
    pub options: serde_json::Value,
}
fn interval() -> u64 {
    60
}
fn options() -> serde_json::Value {
    serde_json::json!({})
}
/// @plugin-api
/// @since 2.0
/// @permission host-only
/// @stability stable
/// Separate from plugin code and application config; absent file means no plugins enabled.
#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginConfiguration {
    pub code_root: Option<PathBuf>,
    #[serde(default)]
    pub plugins: BTreeMap<String, PluginApproval>,
}
/// @plugin-api
/// @since 2.0
/// @stability stable
/// Validated in-memory source; no live path is exposed to JavaScript.
pub struct LoadedPlugin {
    pub grant: PluginGrant,
    pub source: String,
    pub approval: PluginApproval,
}

/// @plugin-api
/// @since 2.0
/// @stability stable
/// Read-only repository; implementations never execute code or expand host approvals.
pub trait PluginRepository: Send + Sync {
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    fn configuration(&self) -> PluginResult<PluginConfiguration>;
    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    fn load(&self, id: &str) -> PluginResult<LoadedPlugin>;
}
