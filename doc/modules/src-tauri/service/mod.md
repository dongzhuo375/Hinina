# mod

## 职责
服务层模块路由文件。声明并暴露所有子服务模块（auth、contest、problem、submission、workspace、config、theme），作为 `service` crate 模块树的入口。

## 核心类型/函数
- `pub mod auth` — 认证服务模块声明
- `pub mod contest` — 比赛服务模块声明
- `pub mod problem` — 题目服务模块声明
- `pub mod submission` — 提交服务模块声明
- `pub mod workspace` — 工作区服务模块声明
- `pub mod config` — 配置服务模块声明
- `pub mod theme` — 主题服务模块声明

## 直接依赖
无（仅包含 `pub mod` 声明，不 import 任何外部模块）

## 被依赖
- `src-tauri/src/main.rs`（通过 `use crate::service::...` 引用各子服务）
- `src-tauri/src/core/context.rs`（引用 `ConfigService`、`WorkspaceManager`）

## 逻辑流程
无（纯模块声明）
