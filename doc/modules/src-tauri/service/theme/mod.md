# mod

## 职责
主题服务模块入口。负责配色方案切换、编辑器主题自动匹配及主题配置查询。主题配置持久化委托给 `ConfigService`，ThemeService 仅负责切换逻辑并通过 `CoreEventBus` 发布 `CoreEvent::ThemeChanged`（通知其他观察者；前端主题由 IPC 返回值直接应用）。

## 核心类型/函数
- `pub mod error` — 主题错误类型模块声明
- **`ThemeService<R: ConfigRepository>`** — 主题管理服务
  - `fn new(config, event_bus) -> Self` — 创建实例
  - `fn current_theme(&self) -> String` — 获取当前主题名称（light / dark）
  - `fn current_editor_theme(&self) -> String` — 获取当前 Monaco 编辑器主题名
  - `fn get_theme_config(&self) -> ThemeConfig` — 获取当前主题配置完整副本
  - `fn set_theme(&self, theme_name) -> AppResult<()>` — 切换主题并自动匹配编辑器主题（light → "vs"，其余 → "vs-dark"）；非内置主题仅警告但仍允许切换（为未来自定义主题预留扩展点）；发布 `CoreEvent::ThemeChanged`。主题落盘由 `ConfigService::update` 显式完成（失败会返回错误且不发事件），因此 `ThemeChanged` 是「确已落盘」的事实通知，而不是「请去落盘」的命令。**注意**：该方法会一并覆盖 `editor_theme`，而编辑器主题自「解题页编辑器设置」起已可独立选择（`theme_name = light` + `editor_theme = vs-dark` 是合法组合）—— 前端若日后接入整机主题切换（当前前端无 theme bridge、无调用方），需改为不覆盖 `editor_theme` 或先征询用户
  - `fn list_themes(&self) -> &[&str]` — 返回内置主题列表 `["light", "dark"]`
- **字段**：`config: Arc<ConfigService<R>>`, `event_bus: Arc<CoreEventBus>`
- 常量：`BUILTIN_THEMES: &[&str] = &["light", "dark"]`

## 直接依赖
- `core::entity::config::ThemeConfig`
- `core::event::core_event::CoreEvent`
- `core::event::core_event_bus::CoreEventBus`
- `core::repository::config_repo::ConfigRepository`
- `service::config::ConfigService`

## 被依赖
- `core::context`（`AppContext` 持有 `Arc<ThemeService<FsConfigRepository>>`）
- `commands::theme_cmd`（`get_theme` / `set_theme`）

## 逻辑流程
- **current_theme / current_editor_theme**：直接从 `ConfigService::get()` 读取配置返回
- **set_theme(name)**：校验名称（非内置仅 `warn`）→ 通过 `ConfigService::update()` 写入 theme_name 和 editor_theme（**失败直接返回，不发事件**）→ 发布 `CoreEvent::ThemeChanged`
- **list_themes**：直接返回内置常量引用，无 I/O
