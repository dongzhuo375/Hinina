# extension

## 职责
定义插件的 UI 扩展点类型体系。插件通过注册扩展点来增强客户端界面，v0.x 仅定义接口，v1.0+ 实现渲染。

## 核心类型/函数
- `ExtensionPoint` — 扩展点枚举，包含 Sidebar、Command、StatusBar、Editor 四种扩展点
- `SidebarExtension` — 侧边栏扩展，携带 id、title、icon
- `CommandExtension` — 命令面板扩展，携带 id、label、可选的 shortcut
- `StatusBarExtension` — 状态栏扩展，携带 id、text、可选的 tooltip
- `EditorExtension` — Monaco Editor 扩展，携带 id、extension_type
- `EditorExtensionType` — 编辑器扩展类型枚举：Completion、Hover、Decoration

## 直接依赖
- `serde::{Deserialize, Serialize}` — 序列化支持

## 被依赖
- `plugin/host/manifest.rs` — `PluginManifest` 的 `extension_points` 字段引用 `ExtensionPoint`

## 逻辑流程
无（纯类型定义）
