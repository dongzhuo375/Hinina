# workspace_cmd

## 职责
工作区相关 Tauri Command 模块。管理代码编辑工作区的加载、保存、切换、状态查询、文件内容同步与语言设置，面向前端暴露 IPC 接口（invoke 名与函数同名，无命名空间前缀）。

## 核心类型/函数
- `pub async fn load_workspace(ctx, contest_id, problem_id) -> AppResult<Workspace>` — 加载或创建指定比赛/题目的工作区（`find_or_create`：按 contest_id + problem_id 匹配已有工作区，命中恢复代码，未命中新建）；首次调用时在 Tauri Command 的 tokio 上下文中懒启动 auto-save（P39：确保 `tokio::spawn` 运行在正确 runtime 上）
- `pub async fn save_workspace(ctx) -> AppResult<()>` — 持久化当前工作区（脏时全量写文件 + 元数据）；未脏时无操作且不发布事件，成功时发布 `WorkspaceEvent::Saved`
- `pub async fn switch_workspace(ctx, workspace_id) -> AppResult<Workspace>` — 保存当前 → 加载目标，发布 `WorkspaceEvent::Switched`
- `pub async fn current_workspace(ctx) -> AppResult<Option<Workspace>>` — 获取当前活动工作区（`None` = 无）；WorkspaceManager 未初始化时也返回 `Ok(None)` 而非错误
- `pub async fn update_workspace_file(ctx, file_name, content) -> AppResult<()>` — 前端 Monaco 编辑器内容同步到后端（2s 防抖推送的落点）。**只写内存不落盘**：磁盘写入由 auto-save 与显式 `save_workspace` 负责，这样「自动保存间隔」才真正决定落盘频率
- `pub async fn set_workspace_language(ctx, language) -> AppResult<Workspace>` — 设置当前工作区语言并**立即落盘**。前端 invoke 签名 `set_workspace_language`({ language })。动机：语言不属于任何代码文件，`update_workspace_file` 带不上它；若不单独持久化，切题或重启后会退回默认语言——选手的 Java 代码会被当作 C++ 提交，属于赛场上最难排查的静默故障
- `fn start_auto_save_if_needed(ctx, wm)`（私有）— 以 `static AtomicBool` 保证进程内只启动一次；读取 `config.editor` 的 `auto_save` / `auto_save_interval_secs` 后 `tokio::spawn` 后台任务

## 直接依赖
- `tauri::State`
- `crate::core::context::AppContext`
- `crate::core::entity::workspace::Workspace`
- `crate::core::error::{AppError, AppResult}`

## 被依赖
- `src-tauri/src/commands/mod.rs`（通过 `pub mod workspace_cmd` 声明）
- `src-tauri/src/main.rs`（`generate_handler!` 注册全部六个 command）

## 逻辑流程
1. 进入解题页 → `load_workspace`（find_or_create 恢复/新建 + 懒启动 auto-save）
2. 编辑过程中前端 2s 防抖推送 `update_workspace_file`（**只进后端内存**）；落盘由两条路径负责：后台 auto-save 周期、前端在切题/失焦/离开页面/关窗时编排的 `save_workspace`
3. 切换语言 → `set_workspace_language` → `WorkspaceManager::set_language`（更新内存 + 立即持久化 `workspace.json` 元数据）
4. 切题 → `switch_workspace` 或再次 `load_workspace`（两者替换当前工作区前都会先落盘旧的）
5. 各 command 从 `ctx.workspace_manager`（`Option<Arc<WorkspaceManager>>`）取管理器，未初始化时返回 `AppError::Workspace`（`current_workspace` 例外，返回 `Ok(None)`）

设计要点：

- **懒启动 auto-save 的已知限制**：`start_auto_save_if_needed` 用 `static AtomicBool` 保证只启动一次，
  且只读取首次 `load_workspace` 时的 `config.editor` —— 设置页改「自动保存开关/间隔」需重启客户端才生效
  （记录于 `doc/problem.md` P74 的范围外项）。
- 落盘失败不影响前端继续编辑：内容仍在后端内存，下一次落盘时机或 auto-save 周期会重写。
