# workspace

## 职责
定义核心领域对象 `Workspace`，负责代码存储、自动保存、崩溃恢复、比赛隔离、模板管理及缓存。工作区按 `(contest_id, problem_id)` 隔离，通过 `HashMap<String, String>` 管理多文件。

## 核心类型/函数
- **`Workspace`** — 工作区 struct，字段：`id`, `contest_id`, `problem_id`, `root_path`, `files`（文件名→内容映射）, `language`, `is_dirty`

## 直接依赖
- `serde::{Deserialize, Serialize}`
- `std::collections::HashMap`

## 被依赖
- `service::workspace::manager`（WorkspaceManager 管理 Workspace 生命周期）
- `commands::workspace_cmd`

## 逻辑流程
无（纯类型定义）。
