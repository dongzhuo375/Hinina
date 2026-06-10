# theme_cmd

## 职责
主题相关 Tauri Command 模块。提供主题配置的读取和设置功能，并定义 `ThemeConfig` 数据结构，面向前端暴露 `theme:*` 命名空间的 IPC 接口。

## 核心类型/函数
- `pub struct ThemeConfig` — 主题配置数据结构（mode, primary_color, font_family, border_radius）
- `pub async fn get_theme(ctx) -> AppResult<ThemeConfig>` — 获取当前主题配置
- `pub async fn set_theme(ctx, config) -> AppResult<()>` — 设置新主题

## 直接依赖
- `serde::{Deserialize, Serialize}`
- `tauri::State`
- `crate::core::context::AppContext`
- `crate::core::error::AppResult`

## 被依赖
- `src-tauri/src/commands/mod.rs`（通过 `pub mod theme_cmd` 声明）

## 逻辑流程
1. 启动时前端调用 `invoke('theme:get')` 读取当前主题，应用到 UI
2. 用户切换主题后，前端调用 `invoke('theme:set', {...})` 持久化
3. `ThemeConfig` 通过 serde 序列化/反序列化在前后端间传递
4. 当前为占位实现（`todo!()`），待接入 ThemeService
