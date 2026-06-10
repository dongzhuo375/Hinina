# config_cmd

## 职责
应用配置相关 Tauri Command 模块。提供客户端配置的读取和更新功能，并定义 `AppConfig` 数据结构，面向前端暴露 `config:*` 命名空间的 IPC 接口。

## 核心类型/函数
- `pub struct AppConfig` — 应用配置数据结构（current_oj, auto_save_interval_ms, theme, language, editor_font_size）
- `pub async fn get_config(ctx) -> AppResult<AppConfig>` — 获取当前应用配置
- `pub async fn update_config(ctx, config) -> AppResult<()>` — 更新应用配置

## 直接依赖
- `serde::{Deserialize, Serialize}`
- `tauri::State`
- `crate::core::context::AppContext`
- `crate::core::error::AppResult`

## 被依赖
- `src-tauri/src/commands/mod.rs`（通过 `pub mod config_cmd` 声明）

## 逻辑流程
1. 前端调用 `invoke('config:get')` 获取当前配置用于设置页面展示
2. 用户修改设置后，前端调用 `invoke('config:update', {...})` 持久化新配置
3. `AppConfig` 通过 serde 序列化/反序列化在前后端间传递
4. 当前为占位实现（`todo!()`），待接入 ConfigService
