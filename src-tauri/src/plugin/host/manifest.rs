//! @plugin-module
//! @layer plugin/host
//! @version 2
//! @stability stable
//!
//! 插件清单的纯数据契约与验证；不读取入口文件，不授予权限。

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

/// @plugin-api
/// @since 1.0
/// @stability stable
/// @security redacted
/// 清单声明的能力；声明不等于宿主授权。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginPermission {
    #[serde(rename = "events.subscribe")]
    EventsSubscribe,
    #[serde(rename = "submission.read")]
    SubmissionRead,
    #[serde(rename = "contest.read")]
    ContestRead,
    #[serde(rename = "identity.read")]
    IdentityRead,
    #[serde(rename = "storage.read")]
    StorageRead,
    #[serde(rename = "storage.write")]
    StorageWrite,
    #[serde(rename = "network.request")]
    NetworkRequest,
    #[serde(rename = "notification")]
    Notification,
}

/// @plugin-api
/// @since 1.0
/// @stability stable
/// 插件可声明的事件名。Resync 是强制控制通知，不依赖声明。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginEventSubscription {
    #[serde(rename = "system.oj_switched")]
    OjSwitched,
    #[serde(rename = "contest.selected")]
    ContestSelected,
    #[serde(rename = "announcement.changed")]
    AnnouncementChanged,
    #[serde(rename = "problem.opened")]
    ProblemOpened,
    #[serde(rename = "submission.created")]
    SubmissionCreated,
    #[serde(rename = "submission.judged")]
    SubmissionJudged,
    #[serde(rename = "workspace.saved")]
    WorkspaceSaved,
    #[serde(rename = "system.config_changed")]
    ConfigChanged,
    #[serde(rename = "system.theme_changed")]
    ThemeChanged,
    #[serde(rename = "session.logged_in")]
    SessionLoggedIn,
    #[serde(rename = "session.logged_out")]
    SessionLoggedOut,
    #[serde(rename = "session.expired")]
    SessionExpired,
    #[serde(rename = "resync.required")]
    ResyncRequired,
}

impl PluginEventSubscription {
    /// @plugin-api
    /// @since 1.0
    /// @stability stable
    /// 稳定日志和订阅名称，与序列化名称一致。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OjSwitched => "system.oj_switched",
            Self::ContestSelected => "contest.selected",
            Self::AnnouncementChanged => "announcement.changed",
            Self::ProblemOpened => "problem.opened",
            Self::SubmissionCreated => "submission.created",
            Self::SubmissionJudged => "submission.judged",
            Self::WorkspaceSaved => "workspace.saved",
            Self::ConfigChanged => "system.config_changed",
            Self::ThemeChanged => "system.theme_changed",
            Self::SessionLoggedIn => "session.logged_in",
            Self::SessionLoggedOut => "session.logged_out",
            Self::SessionExpired => "session.expired",
            Self::ResyncRequired => "resync.required",
        }
    }
}

/// @plugin-api
/// @since 1.0
/// @permission network.request
/// @stability stable
/// 网络执行上限声明；本类型验证不执行 DNS 或 HTTP。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginNetworkPolicy {
    pub allow_origins: Vec<String>,
    pub max_body_bytes: u32,
    pub timeout_ms: u32,
}

impl Default for PluginNetworkPolicy {
    fn default() -> Self {
        Self {
            allow_origins: vec![],
            max_body_bytes: 65_536,
            timeout_ms: 5_000,
        }
    }
}

/// @plugin-api
/// @since 1.0
/// @stability experimental
/// 预留扩展点；本轮不接受非空 contributes。
pub type PluginContribution = serde_json::Value;

/// @plugin-api
/// @since 1.0
/// @stability stable
/// @security redacted
/// 版本化清单。未知字段、权限或事件在反序列化时拒绝。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub manifest_version: u32,
    pub runtime: String,
    pub entry: String,
    pub api_version: u32,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub min_app_version: Option<String>,
    pub permissions: Vec<PluginPermission>,
    pub events: Vec<PluginEventSubscription>,
    #[serde(default)]
    pub network: PluginNetworkPolicy,
    #[serde(default)]
    pub contributes: Vec<PluginContribution>,
    #[serde(default = "empty_object")]
    pub config_schema: serde_json::Value,
}

fn empty_object() -> serde_json::Value {
    serde_json::json!({})
}

/// @plugin-api
/// @since 1.0
/// @stability stable
/// 安全清单错误，只标识字段，不返回输入值或绝对路径。
#[derive(Debug, Error, PartialEq, Eq)]
#[error("invalid plugin manifest field: {0}")]
pub struct ManifestError(pub &'static str);

impl PluginManifest {
    /// @plugin-api
    /// @since 1.0
    /// @stability stable
    /// 验证纯清单；入口存在性/符号链接边界由 FsPluginRepository 验证。
    /// # Failure
    /// 返回 ManifestError，不加载代码或授予能力。
    pub fn validate(&self) -> Result<(), ManifestError> {
        let valid_id = !self.id.is_empty()
            && self.id.len() <= 128
            && self.id.split('.').all(|part| {
                !part.is_empty()
                    && part.as_bytes()[0].is_ascii_lowercase()
                    && part
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            });
        if !valid_id {
            return Err(ManifestError("id"));
        }
        if self.name.trim().is_empty() || self.name.len() > 128 {
            return Err(ManifestError("name"));
        }
        if !valid_version(&self.version) {
            return Err(ManifestError("version"));
        }
        if self
            .min_app_version
            .as_ref()
            .is_some_and(|v| !valid_version(v))
        {
            return Err(ManifestError("minAppVersion"));
        }
        if self.manifest_version != 2 {
            return Err(ManifestError("manifestVersion"));
        }
        if self.api_version != 2 {
            return Err(ManifestError("apiVersion"));
        }
        if self.runtime != "js" {
            return Err(ManifestError("runtime"));
        }
        if self.entry.is_empty()
            || self.entry.len() > 256
            || self.entry.contains('\\')
            || self.entry.contains(':')
            || self.entry.contains('\0')
            || self.entry.split('/').any(|p| {
                p.is_empty() || p == "." || p == ".." || p.ends_with('.') || p.ends_with(' ')
            })
            || !self.entry.ends_with(".js")
        {
            return Err(ManifestError("entry"));
        }
        if self.permissions.iter().collect::<HashSet<_>>().len() != self.permissions.len() {
            return Err(ManifestError("permissions"));
        }
        if self.events.iter().collect::<HashSet<_>>().len() != self.events.len()
            || (!self.events.is_empty()
                && !self
                    .permissions
                    .contains(&PluginPermission::EventsSubscribe))
        {
            return Err(ManifestError("events"));
        }
        if !(1..=65_536).contains(&self.network.max_body_bytes)
            || !(1..=5_000).contains(&self.network.timeout_ms)
            || self.network.allow_origins.len() > 32
            || self.network.allow_origins.iter().any(|h| !valid_origin(h))
            || self
                .network
                .allow_origins
                .iter()
                .collect::<HashSet<_>>()
                .len()
                != self.network.allow_origins.len()
            || (self.permissions.contains(&PluginPermission::NetworkRequest)
                && self.network.allow_origins.is_empty())
            || (!self.network.allow_origins.is_empty()
                && !self.permissions.contains(&PluginPermission::NetworkRequest))
        {
            return Err(ManifestError("network"));
        }
        if !self.contributes.is_empty() {
            return Err(ManifestError("contributes"));
        }
        if self.config_schema != empty_object() {
            return Err(ManifestError("configSchema"));
        }
        Ok(())
    }
}

/// @plugin-api
/// @since 2.0
/// @stability stable
/// Exact normalized HTTP(S) origin, never a path, credentials, wildcard or query.
pub fn valid_origin(origin: &str) -> bool {
    url::Url::parse(origin).is_ok_and(|url| {
        matches!(url.scheme(), "https" | "http")
            && matches!(url.host(), Some(url::Host::Domain(host)) if !host.contains('*'))
            && url.username().is_empty()
            && url.password().is_none()
            && url.path() == "/"
            && url.query().is_none()
            && url.fragment().is_none()
            && url.origin().ascii_serialization() == origin
            && origin.len() <= 512
    })
}

fn valid_version(version: &str) -> bool {
    version.len() <= 128 && semver::Version::parse(version).is_ok()
}

#[cfg(test)]
#[path = "tests/manifest_tests.rs"]
mod tests;
