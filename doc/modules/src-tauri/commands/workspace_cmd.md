# workspace_cmd

## 职责
工作区相关 Tauri Command 模块。管理代码编辑工作区的加载、保存、切换、状态查询、文件内容同步与语言设置，面向前端暴露 IPC 接口（invoke 名与函数同名，无命名空间前缀）。

## 核心类型/函数
- `pub async fn load_workspace(ctx, contest_id, problem_id) -> AppResult<Workspace>` — 加载或创建指定比赛/题目的工作区（`find_or_create`：按 contest_id + problem_id 匹配已有工作区，命中恢复代码，未命中新建）；加载前调用 `sync_auto_save_with_config` 按当前配置同步 auto-save（P39：确保 `tokio::spawn` 运行在正确 runtime 上；P48：不再用一次性 static 标记）
- `pub async fn save_workspace(ctx) -> AppResult<()>` — 持久化当前工作区（脏时全量写文件 + 元数据）；未脏时无操作且不发布事件，成功时发布 `WorkspaceEvent::Saved`
- `pub async fn switch_workspace(ctx, workspace_id) -> AppResult<Workspace>` — 保存当前 → 加载目标，发布 `WorkspaceEvent::Switched`
- `pub async fn current_workspace(ctx) -> AppResult<Option<Workspace>>` — 获取当前活动工作区（`None` = 无）；WorkspaceManager 未初始化时也返回 `Ok(None)` 而非错误
- `pub async fn update_workspace_file(ctx, file_name, content) -> AppResult<()>` — 前端 Monaco 编辑器内容同步到后端（2s 防抖推送的落点）。**只写内存不落盘**：磁盘写入由 auto-save 与显式 `save_workspace` 负责，这样「自动保存间隔」才真正决定落盘频率
- `pub async fn set_workspace_language(ctx, language) -> AppResult<Workspace>` — 设置当前工作区语言并**立即落盘**。前端 invoke 签名 `set_workspace_language`({ language })。动机：语言不属于任何代码文件，`update_workspace_file` 带不上它；若不单独持久化，切题或重启后会退回默认语言——选手的 Java 代码会被当作 C++ 提交，属于赛场上最难排查的静默故障
- `pub fn auto_save_action(auto_save, interval_secs, current_interval) -> AutoSaveAction` — **纯函数判据**：`Keep` / `Start(秒)` / `Stop`。已在运行的间隔与目标一致时 `Keep`（避免每次加载都重置计时器），间隔变了才重启，配置关闭或间隔为 0 则 `Stop`。抽成纯函数是为了让「配置改动能否生效」这条链路可被单测逐格锁定
- `pub fn sync_auto_save_with_config(editor, wm)` — 按当前配置**幂等**地同步 auto-save 的启停与间隔。调用点两处：`load_workspace` 与 `main.rs` 的 `SystemEvent::ConfigReloaded` 订阅。必须在 tokio runtime 上下文里调用（`start_auto_save` 内部用 `tokio::spawn`）

## 直接依赖
- `tauri::State`
- `crate::core::context::AppContext`
- `crate::core::entity::workspace::Workspace`
- `crate::core::error::{AppError, AppResult}`

## 被依赖
- `src-tauri/src/commands/mod.rs`（通过 `pub mod workspace_cmd` 声明）
- `src-tauri/src/main.rs`（`generate_handler!` 注册全部六个 command）

## 逻辑流程
1. 进入解题页 → `load_workspace`（find_or_create 恢复/新建 + 按配置同步 auto-save）
2. 编辑过程中前端 2s 防抖推送 `update_workspace_file`（**只进后端内存**）；落盘由两条路径负责：后台 auto-save 周期、前端在切题/失焦/离开页面/关窗时编排的 `save_workspace`
3. 切换语言 → `set_workspace_language` → `WorkspaceManager::set_language`（更新内存 + 立即持久化 `workspace.json` 元数据）
4. 切题 → `switch_workspace` 或再次 `load_workspace`（两者替换当前工作区前都会先落盘旧的）
5. 各 command 从 `ctx.workspace_manager`（`Option<Arc<WorkspaceManager>>`）取管理器，未初始化时返回 `AppError::Workspace`（`current_workspace` 例外，返回 `Ok(None)`）

设计要点：

- **auto-save 的启停是「按配置同步」，不是一次性懒启动**（P48 修复）：旧实现用 `static AtomicBool`
  只启动一次，设置页关掉后再也打不开、间隔也只读首次配置。现在由 `auto_save_action` 的判据表决定
  `Keep` / `Start` / `Stop`，并由 `SystemEvent::ConfigReloaded` 订阅触发热生效 ——
  **改开关/间隔无需重启客户端**。
  **注意事件来源**：`ConfigReloaded` 由 `ConfigService::update` 与 `reload` 两条路径发布；
  只挂 `reload_config` 命令是不够的（它在前端没有调用方，2026-09-21 复核发现的生产断链）。
- 落盘失败不影响前端继续编辑：内容仍在后端内存，下一次落盘时机或 auto-save 周期会重写。
