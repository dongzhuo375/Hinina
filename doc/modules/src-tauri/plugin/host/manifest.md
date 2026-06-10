# manifest

## 职责
定义插件的 Manifest 数据结构，描述插件的元数据、所需权限清单及注册的扩展点，对应插件根目录下的 `manifest.json` 文件。

## 核心类型/函数
- `PluginManifest` — 插件清单结构体，包含 id、name、version、description、author、permissions、extension_points、min_app_version 字段
- `PluginPermission` — 插件权限枚举，当前包含 WorkspaceRead、WorkspaceWrite、ProblemRead、ContestRead、Notification 五种权限

## 直接依赖
- `serde::{Deserialize, Serialize}` — 序列化支持
- `crate::plugin::host::extension::ExtensionPoint` — 扩展点类型

## 被依赖
- `core/repository/plugin_repo.rs` — `PluginRepository` trait 的 `scan_plugins` 方法返回 `Vec<PluginManifest>`
- `infra/fs_plugin_repo.rs` — `FsPluginRepository` 实现中使用 `PluginManifest`

## 逻辑流程
无（纯类型定义）
