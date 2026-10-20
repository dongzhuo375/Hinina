//! @plugin-module
//! @layer plugin/permission
//! @version 1
//! @stability stable
//! Manifest 声明和宿主授权分离；本模块不实现用户授权 UI 或持久化。

use crate::plugin::api::{PluginError, PluginResult};
use crate::plugin::host::manifest::{PluginEventSubscription, PluginManifest, PluginPermission};
use std::collections::HashSet;

/// @plugin-api
/// @since 1.0
/// @stability stable
/// 宿主签发的不可变授权快照，不能从 JS 反序列化或更改字段。
#[derive(Debug, Clone)]
pub struct PluginGrant {
    manifest: PluginManifest,
    permissions: HashSet<PluginPermission>,
}

impl PluginGrant {
    /// @plugin-api
    /// @since 1.0
    /// @permission host-only
    /// @stability stable
    /// 宿主传入已批准的权限。不能授予未声明能力；非法清单返回 InvalidInput。
    pub fn authorize(
        manifest: &PluginManifest,
        approved: &[PluginPermission],
    ) -> PluginResult<Self> {
        manifest.validate().map_err(|_| PluginError::InvalidInput)?;
        if approved.iter().any(|p| !manifest.permissions.contains(p)) {
            return Err(PluginError::InvalidInput);
        }
        Ok(Self {
            manifest: manifest.clone(),
            permissions: approved.iter().copied().collect(),
        })
    }

    /// @plugin-api
    /// @since 1.0
    /// @stability stable
    /// 验证宿主授予权限；Manifest 声明本身不能通过此检查。
    pub fn require(&self, permission: PluginPermission) -> PluginResult<()> {
        if self.permissions.contains(&permission) {
            Ok(())
        } else {
            Err(PluginError::PermissionDenied { permission })
        }
    }

    /// @plugin-api
    /// @since 1.0
    /// @stability stable
    /// 返回宿主验证后的只读清单快照。
    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }
}

impl PluginEventSubscription {
    /// @plugin-api
    /// @since 1.0
    /// @stability stable
    /// 载荷按领域读取权限保护；控制通知/系统刷新不暴露领域细节。
    pub fn required_permission(self) -> Option<PluginPermission> {
        match self {
            Self::SubmissionCreated | Self::SubmissionJudged => {
                Some(PluginPermission::SubmissionRead)
            }
            Self::ContestSelected | Self::AnnouncementChanged | Self::ProblemOpened => {
                Some(PluginPermission::ContestRead)
            }
            // 工作区 API/权限未定义，Host 暂不投递 WorkspaceSaved。
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "tests/permission_tests.rs"]
mod tests;
