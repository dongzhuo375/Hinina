# workspace

## 职责
定义核心领域对象 `Workspace`，负责代码存储、自动保存、崩溃恢复、比赛隔离、模板管理及缓存。工作区按 `(contest_id, problem_id)` 隔离，通过 `HashMap<String, String>` 管理多文件。提供 `new()`、`touch()`、`mark_dirty()`、`mark_clean()` 辅助方法。

## 核心类型/函数
- **`Workspace`** — 工作区 struct，字段：`id`, `contest_id`, `problem_id`, `root_path`, `files`（文件名→内容映射）, `language`, `is_dirty`, `created_at`, `updated_at`
- **`Workspace::new(contest_id, problem_id, root_path)`** — 构造器，自动生成 id 并填充 UTC 时间戳
- **`Workspace::touch(&mut self)`** — 更新 `updated_at` 为当前时间
- **`Workspace::mark_dirty(&mut self)`** — 标记未保存 + 更新 `updated_at`
- **`Workspace::mark_clean(&mut self)`** — 标记已保存

## 直接依赖
- `serde::{Deserialize, Serialize}`
- `std::collections::HashMap`
- `std::time::SystemTime`

## 被依赖
- `service::workspace::manager`（WorkspaceManager 管理 Workspace 生命周期）
- `commands::workspace_cmd`

## 逻辑流程
- **new**：拼接 id 格式 `ws-{contest_id}-{problem_id}-{timestamp}`，created_at 和 updated_at 初始化为当前 UTC 时间戳
- **touch**：仅更新 updated_at，用于崩溃恢复时判断最近活跃工作区
- **mark_dirty**：设置 is_dirty=true 并调 touch 更新时间戳
- **mark_clean**：设置 is_dirty=false（不更新 updated_at，保存由 Service 层控制）

## 测试
测试代码位于 `tests/workspace_tests.rs`，5 项：时间戳初始化、touch 行为、dirty/clean 切换、mark_dirty 触发 touch、id 格式。
