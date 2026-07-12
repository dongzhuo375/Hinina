# mod

## 职责
主题服务模块入口。负责配色方案切换、编辑器主题自动匹配及主题配置查询。主题配置持久化委托给 `ConfigService`，ThemeService 仅负责切换逻辑并通过 EventBus 发布 `SystemEvent::ThemeChanged` 通知前端。

## 核心类型/函数
- `pub mod error` — 主题错误类型模块声明
- **`ThemeService<R: ConfigRepository>`** — 主题管理服务
  - `fn new(config, event_bus) -> Self` — 创建实例
  - `fn current_theme(&self) -> String` — 获取当前主题名称（light / dark）
  - `fn current_editor_theme(&self) -> String` — 获取当前 Monaco 编辑器主题名
  - `fn get_theme_config(&self) -> ThemeConfig` — 获取当前主题配置完整副本
  - `fn set_theme(&self, theme_name) -> AppResult<()>` — 切换主题并自动匹配编辑器主题（light → "vs"，其余 → "vs-dark"）；非内置主题仅警告但仍允许切换；发布 `SystemEvent::ThemeChanged`
  - `fn list_themes(&self) -> &[&str]` — 返回内置主题列表 `["light", "dark"]`
- **字段**：`config: Arc<ConfigService<R>>`, `event_bus: Arc<EventBus>`
- 常量：`BUILTIN_THEMES: &[&str] = &["light", "dark"]`

## 直接依赖
- `core::entity::config::ThemeConfig`
- `core::event::app_event::{AppEvent, SystemEvent}`
- `core::event::event_bus::EventBus`
- `core::repository::config_repo::ConfigRepository`
- `service::config::ConfigService`

## 被依赖
- `core::context`（`AppContext` 持有 `Arc<ThemeService<FsConfigRepository>>`）

## 逻辑流程
- **current_theme / current_editor_theme**：直接从 `ConfigService::get()` 读取配置返回
- **set_theme(name)**：校验名称 → 通过 `ConfigService::update()` 写入 theme_name 和 editor_theme → 发布 `SystemEvent::ThemeChanged`
- **list_themes**：直接返回内置常量引用，无 I/O
