# host/extension

## 职责
保留远期 UI 扩展点类型草案，当前不能注册或渲染，不承诺后续版本排期。

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
暂无执行调用者。Manifest 使用空 `contributes` 预留字段（`PluginContribution = serde_json::Value`），不再引用旧 `extension_points`，非空扩展声明会被拒绝。

## 逻辑流程
无（纯类型定义）。
