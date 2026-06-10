# mod

## 职责
配置服务模块入口。负责用户配置、OJ 配置、编辑器偏好、主题、布局的统一管理。当前为骨架阶段，仅声明 `error` 子模块；`ConfigService` 类型预期在此文件中定义但尚未实现。

## 核心类型/函数
- `pub mod error` — 配置错误类型模块声明
- `ConfigService`（预期类型，当前未定义，`context.rs` 中已预留引用）

## 直接依赖
无（仅包含 `pub mod error` 声明）

## 被依赖
- `src-tauri/src/core/context.rs`（`AppContext` 持有 `Arc<ConfigService>`）
- `src-tauri/src/main.rs`（注释中引用 ConfigService 初始化顺序）

## 逻辑流程
无（纯模块声明，`ConfigService` 类型及业务逻辑待实现）
