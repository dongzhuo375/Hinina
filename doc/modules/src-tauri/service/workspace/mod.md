# mod

## 职责
工作区服务模块入口。负责文件读写、自动保存、崩溃恢复、比赛隔离。当前为骨架阶段，声明 `error` 和 `manager` 子模块。

## 核心类型/函数
- `pub mod error` — 工作区错误类型模块声明
- `pub mod manager` — 工作区管理器模块声明

## 直接依赖
无（仅包含 `pub mod` 声明）

## 被依赖
- `src-tauri/src/core/context.rs`（通过 `crate::service::workspace::manager::WorkspaceManager` 引用）

## 逻辑流程
无（纯模块声明）
