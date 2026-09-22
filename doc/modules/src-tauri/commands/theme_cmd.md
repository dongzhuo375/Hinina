# theme_cmd

## 职责
主题相关 Tauri Command 模块。提供主题配置读取与切换，面向前端暴露 IPC 接口。均为 `ThemeService` 的薄封装（不含业务逻辑）。

## 核心类型/函数
- `pub async fn get_theme(ctx) -> AppResult<ThemeConfig>` — 获取当前主题配置。前端 invoke 签名 `get_theme`；返回 `ThemeConfig`（主题名称 + 编辑器主题），从 `ConfigService` 读取（`ctx.theme.get_theme_config()`）
- `pub async fn set_theme(ctx, theme_name: String) -> AppResult<()>` — 切换主题。前端 invoke 签名 `set_theme`({ themeName })；支持 `"light"` / `"dark"`，自动匹配 Monaco 编辑器主题（`vs` / `vs-dark`）。`ThemeService::set_theme` 经 `ConfigService::update` **显式落盘**（失败直接返回、不发事件），成功后发布 `CoreEvent::ThemeChanged` 通知其他观察者（前端主题由 IPC 返回值直接应用）
- `ThemeConfig` 定义在 `core::entity::config`（本模块不重复定义）

## 直接依赖
- `tauri::State`
- `tracing::info`
- `crate::core::context::AppContext`
- `crate::core::entity::config::ThemeConfig`
- `crate::core::error::AppResult`

## 被依赖
- `src-tauri/src/commands/mod.rs`（通过 `pub mod theme_cmd` 声明）
- `src-tauri/src/main.rs`（`generate_handler!` 注册 `get_theme` / `set_theme`）

## 逻辑流程
1. 前端调用 `invoke('get_theme')` 读取当前主题配置并应用到 UI
2. 用户切换主题后，前端调用 `invoke('set_theme', { themeName })`：`ThemeService` 经 `ConfigService::update` 落盘（含自动匹配的 `editor_theme`）→ 发布 `CoreEvent::ThemeChanged`
3. `ThemeConfig` 通过 serde 序列化在前后端间传递

> 前端当前没有 theme bridge（无调用方），命令已就绪、待接入。
