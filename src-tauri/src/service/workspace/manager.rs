use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use tracing::{debug, info, warn};

use crate::core::entity::workspace::Workspace;
use crate::core::error::{AppError, AppResult};
use crate::core::event::core_event::CoreEvent;
use crate::core::event::core_event_bus::CoreEventBus;
use crate::core::repository::workspace_repo::WorkspaceRepository;
use serde::{Deserialize, Serialize};

/// Workspace 管理器。
///
/// 管理 Workspace 完整生命周期：
/// create → load → save → auto-save → switch → destroy → recover
///
/// ## 生命周期状态机
/// ```text
///           create
///             │
///             ▼
///   ┌──────────────────┐
///   │     Created       │
///   └──────┬───────────┘
///          │ load / recover
///          ▼
///   ┌──────────────────┐
///   │     Active        │◄──── switch
///   │  (auto-saving)    │
///   └──────┬───────────┘
///          │
///     ┌────┴────┐
///     ▼         ▼
///   Saved    Destroyed
///   (idle)   (removed)
/// ```
pub struct WorkspaceManager {
    repo: Arc<dyn WorkspaceRepository>,
    event_bus: Arc<CoreEventBus>,
    /// 当前活动工作区（RwLock 内直接持有 Workspace，支持 auto-save 共享和可变访问）
    current: Arc<RwLock<Option<Workspace>>>,
    /// 内容修订号：每次 `update_file` 在**写锁内**递增。
    ///
    /// auto-save 取快照时一并记录修订号，写盘后仅当修订号未变才 `mark_clean` ——
    /// 否则「快照写盘」会被误当成本次编辑已落盘，最新内容将停留在内存直到下一次编辑。
    revision: Arc<AtomicU64>,
    /// 自动保存的 JoinHandle
    auto_save_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// 当前 auto-save 的间隔（秒）；`None` = 未运行。
    ///
    /// 单独记一份状态是为了让「按配置同步」成为**幂等**操作：调用方据此判断
    /// 「要不要重启」，避免每次 `load_workspace` 都重置计时器，也避免「关掉再打开」
    /// 时无状态可依（旧实现用 `static AtomicBool` 做一次性懒启动，关掉后再也起不来）。
    auto_save_interval_secs: Mutex<Option<u64>>,
}

/// 工作区元数据，持久化在 workspace.json 中，避免从 workspace_id 字符串解析字段。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkspaceMeta {
    contest_id: String,
    problem_id: String,
    root_path: String,
    /// 当前代码文件名（权威源，见 `Workspace::active_file`）。
    /// `#[serde(default)]`：历史 workspace.json 没有该字段，反序列化为 `None`，
    /// 由加载路径回退到「按语言派生 + 扩展名探测」的老启发式。
    #[serde(default)]
    active_file: Option<String>,
    language: String,
    created_at: i64,
    updated_at: i64,
}

/// auto-save 写盘后能否把工作区标记为 clean 的判据。
///
/// 两个条件都必须成立：**仍是同一个工作区**（切题后不得清新工作区的脏标记）
/// 与**修订号未变**（快照之后没有新改动）。后者是「快照写盘 ≠ 本次编辑已落盘」
/// 的唯一防线：少了它，写盘期间/之后到来的改动会被静默吞掉。
///
/// 独立成纯函数是为了**确定性可测**：集成测试里「快照之后到来新改动」的窗口由
/// 锁竞争决定（写盘持读锁，无法在同线程注入；跨线程注入则胜负不定，实测事件数
/// 在 1/2 间浮动），因此判据的削弱只能由本函数的单测稳定证伪。
fn can_mark_clean(
    ws_id: &str,
    current_id: &str,
    snapshot_revision: u64,
    current_revision: u64,
) -> bool {
    ws_id == current_id && snapshot_revision == current_revision
}

impl WorkspaceManager {
    /// 创建 WorkspaceManager。
    pub fn new(repo: Arc<dyn WorkspaceRepository>, event_bus: Arc<CoreEventBus>) -> Self {
        Self {
            repo,
            event_bus,
            current: Arc::new(RwLock::new(None)),
            revision: Arc::new(AtomicU64::new(0)),
            auto_save_handle: Mutex::new(None),
            auto_save_interval_secs: Mutex::new(None),
        }
    }

    // ── 生命周期方法 ──

    /// 创建新工作区，初始化空文件集合并设为当前。
    ///
    /// 替换当前工作区前先落盘旧的（内存是唯一权威副本，见 [`Self::save_current_if_dirty`]）。
    ///
    /// **不发布事件**：工作区的创建由前端经 IPC 发起，返回值即真值；
    /// 「已创建」不是需要其他观察者知晓的事实（旧实现发布 `Loaded`，无任何消费者）。
    pub fn create(
        &self,
        contest_id: &str,
        problem_id: &str,
        root_path: &str,
    ) -> AppResult<Workspace> {
        self.save_current_if_dirty();

        let ws = Workspace::new(
            contest_id.to_string(),
            problem_id.to_string(),
            root_path.to_string(),
        );

        // 持久化元数据到 workspace.json，避免从 workspace_id 字符串解析元数据
        self.persist_meta(&ws)?;

        {
            let mut current = self.current.write().unwrap_or_else(|e| e.into_inner());
            *current = Some(ws.clone());
        }

        info!(
            workspace_id = ws.id,
            contest_id = contest_id,
            problem_id = problem_id,
            "工作区已创建"
        );

        Ok(ws)
    }

    /// 加载已有工作区：从磁盘恢复所有文件到内存。
    ///
    /// 替换当前工作区前先落盘旧的（内存是唯一权威副本，见 [`Self::save_current_if_dirty`]）。
    /// 重新加载同一工作区时，这一步同时保证「刚推送到内存的内容」先落盘再被读回。
    ///
    /// **不发布事件**：加载由前端经 IPC 发起，返回值即真值。
    pub fn load(&self, workspace_id: &str, _root_path: &str) -> AppResult<Workspace> {
        if !self.repo.exists(workspace_id) {
            return Err(AppError::Workspace(format!(
                "工作区不存在: {}",
                workspace_id
            )));
        }

        // 替换 current 之前落盘旧的：同一工作区重载时先落盘再读回，
        // 保证「刚推送到内存的内容」出现在本次读取结果里（否则读回旧内容）
        self.save_current_if_dirty();

        let file_paths = self.repo.list_files(workspace_id)?;

        // 读取元数据
        let meta_json = self
            .repo
            .read_file(workspace_id, &PathBuf::from("workspace.json"))?;
        let meta: WorkspaceMeta = serde_json::from_str(&meta_json)
            .map_err(|e| AppError::Serialization(format!("解析工作区元数据失败: {}", e)))?;

        let mut files = HashMap::new();

        // 恢复所有文件，跳过元数据文件本身
        for path in &file_paths {
            // 跳过 workspace.json，避免将其作为用户文件加载
            if path == &PathBuf::from("workspace.json") {
                continue;
            }
            match self.repo.read_file(workspace_id, path) {
                Ok(content) => {
                    files.insert(path.to_string_lossy().into_owned(), content);
                }
                Err(e) => {
                    warn!(
                        workspace_id = workspace_id,
                        file = %path.display(),
                        error = %e,
                        "恢复文件失败"
                    );
                }
            }
        }

        let ws = Workspace {
            id: workspace_id.to_string(),
            contest_id: meta.contest_id,
            problem_id: meta.problem_id,
            root_path: meta.root_path,
            files,
            active_file: meta.active_file,
            language: meta.language,
            is_dirty: false,
            created_at: meta.created_at,
            updated_at: meta.updated_at,
        };

        {
            let mut current = self.current.write().unwrap_or_else(|e| e.into_inner());
            *current = Some(ws.clone());
        }

        info!(workspace_id = workspace_id, "工作区已加载");

        Ok(ws)
    }

    /// 保存当前工作区到磁盘（脏时**全量**写入 `files` 中的所有文件 + 元数据）。
    ///
    /// 落盘的唯一同步入口：`update_file` 只写内存，磁盘写入由本方法（前端在切题 /
    /// 失焦 / 关窗时调用）与 auto-save 周期负责。工作区脏标记是整体粒度的，
    /// 不区分单文件 —— 逐个跟踪需为每个文件维护独立脏标记，当前工作区通常只有
    /// 一个源文件，收益不抵复杂度。
    ///
    /// 持有写锁完成写盘（保存期间不接受 `update_file`），因此不会与编辑器同步竞争；
    /// 未脏时直接返回且**不发布** `CoreEvent::WorkspaceSaved` —— 该事件等价于
    /// 「最新内容确已在磁盘上」，不能为一次空操作发布。
    pub fn save(&self) -> AppResult<()> {
        let mut current = self.current.write().unwrap_or_else(|e| e.into_inner());
        let ws = match current.as_mut() {
            Some(ws) => ws,
            None => {
                debug!("无当前工作区，跳过保存");
                return Ok(());
            }
        };

        if !ws.is_dirty {
            return Ok(());
        }

        let mut saved = 0usize;
        for (file_name, content) in &ws.files {
            let path = PathBuf::from(file_name);
            self.repo.save_file(&ws.id, &path, content)?;
            saved += 1;
        }

        // 元数据随保存一并落盘：语言等字段不属于任何代码文件，
        // 只写文件会让它们永远停留在创建时的初值
        self.persist_meta(ws)?;

        ws.mark_clean();

        debug!(workspace_id = ws.id, files_saved = saved, "工作区已保存");

        let ws_id = ws.id.clone();
        // 修订号必须在**写锁内**取：释放锁之后 `update_file` 可能立刻递增它，
        // 那样事件会报出比磁盘内容更新的修订号，前端据此清脏 = 假 clean
        // （最新内容仍在内存，直到下一次编辑才可能落盘）。
        let revision = self.revision.load(Ordering::SeqCst);
        drop(current);

        // 显式落盘已完成，此处只发布「确已落盘」的事实
        self.event_bus.publish(CoreEvent::WorkspaceSaved {
            workspace_id: ws_id,
            revision,
            automatic: false,
        });

        Ok(())
    }

    /// 设置当前工作区的编程语言，并**立即**持久化元数据。
    ///
    /// 语言不属于任何代码文件，`update_file` 与「仅写文件」的保存路径都带不上它，
    /// 因此必须单独落盘。否则切换语言后切题或重启客户端会退回默认语言 ——
    /// 选手的 Java 代码会被当作 C++ 提交，属于赛场上最难排查的静默故障。
    ///
    /// 不修改脏标记：语言变更不需要重写代码文件。
    pub fn set_language(&self, language: &str) -> AppResult<Workspace> {
        let ws = {
            let mut current = self.current.write().unwrap_or_else(|e| e.into_inner());
            let ws = current
                .as_mut()
                .ok_or_else(|| AppError::Workspace("无当前工作区".into()))?;
            ws.language = language.to_string();
            ws.touch();
            ws.clone()
        };

        self.persist_meta(&ws)?;

        debug!(
            workspace_id = ws.id,
            language = language,
            "工作区语言已更新并落盘"
        );
        Ok(ws)
    }

    /// 把工作区元数据写入 `workspace.json`（create / save / set_language 共用）。
    ///
    /// **auto-save 刻意不写元数据**：`set_language` 会立即落盘语言，若 auto-save
    /// 用「快照时刻的元数据」回写，会把刚落盘的新语言覆盖回旧值（快照与写入之间的
    /// 语言变更无从察觉）。元数据由显式落盘路径维护。
    fn persist_meta(&self, ws: &Workspace) -> AppResult<()> {
        let meta = WorkspaceMeta {
            contest_id: ws.contest_id.clone(),
            problem_id: ws.problem_id.clone(),
            root_path: ws.root_path.clone(),
            active_file: ws.active_file.clone(),
            language: ws.language.clone(),
            created_at: ws.created_at,
            updated_at: ws.updated_at,
        };
        let meta_json = serde_json::to_string_pretty(&meta)
            .map_err(|e| AppError::Serialization(format!("序列化工作区元数据失败: {}", e)))?;
        self.repo
            .save_file(&ws.id, &PathBuf::from("workspace.json"), &meta_json)
    }

    /// 落盘当前工作区（脏时才写），失败仅告警。
    ///
    /// `create` / `load` / `switch` 都会替换 `current`，而内存是唯一权威副本
    /// （`update_file` 不再写盘）：替换前必须先落盘旧工作区，否则未落盘的改动
    /// 随替换静默消失。失败不阻断替换 —— 崩溃恢复与重新加载仍可用旧内容兜底，
    /// 但必须在日志里留下痕迹。
    fn save_current_if_dirty(&self) {
        if let Err(e) = self.save() {
            warn!(error = %e, "替换当前工作区前保存失败");
        }
    }

    /// 启动后台自动保存。
    ///
    /// 如果已有自动保存任务运行，则先停止旧的再启动。
    ///
    /// **「停旧的 → 起新的 → 登记句柄」必须在一把锁内完成**：三者分三次取锁时，
    /// 并发的两个调用会各自停掉「当时存在的」任务、各自 spawn，随后**后登记者的
    /// 句柄覆盖先登记者** —— 先起的循环就此成为无人可停的孤儿任务，`stop_auto_save`
    /// 之后仍在按自己的节拍写盘（表现为「关了自动保存却还在保存」）。
    ///
    /// 循环语义（配合 `update_file` 只写内存）：
    /// - 脏才写，且**取快照与写盘整体在读锁内完成**（与 `save()` / `update_file`
    ///   的写锁互斥）：只锁住「取快照」会让写盘期间到来的 `save()` 插进本次写盘与
    ///   后续检查之间 —— 新内容先落盘，被延迟的旧快照写再覆盖回旧内容，而工作区
    ///   已由 `save()` 标 clean（rev 检查只能阻止 auto-save 误标 clean，无法撤销
    ///   已发生的覆盖）→ 新内容永不重写，静默永久丢失；
    /// - 写盘失败保持脏，下一 tick 重试，且**不发布事件** —— 前端「已自动备份」
    ///   必须表示最新内容确已落盘；
    /// - 仅当快照之后没有新改动（修订号未变）才 `mark_clean` 并发布
    ///   `CoreEvent::WorkspaceSaved { automatic: true }`；有新改动时保留脏标记，下轮重写。
    pub fn start_auto_save(&self, interval_secs: u64) {
        // 锁序固定为 handle → interval（`stop_auto_save` 同序），无死锁面。
        // 持锁期间只做「abort + spawn + 登记」：spawn 不阻塞，且循环体不取这把锁。
        let mut handle = self
            .auto_save_handle
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(previous) = handle.take() {
            previous.abort();
        }

        // auto-save 通过 Arc 共享 current 与修订号，安全且 Send。
        let repo = Arc::clone(&self.repo);
        let event_bus = Arc::clone(&self.event_bus);
        let current = Arc::clone(&self.current);
        let revision = Arc::clone(&self.revision);

        let task = tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(Duration::from_secs(interval_secs));
            // 跳过首次立即触发
            interval_timer.tick().await;

            loop {
                interval_timer.tick().await;

                // 取快照 + 写盘整体在读锁内完成（见方法文档）：写盘期间
                // save()/update_file 无法插入，保证「旧快照的写」不可能落在
                // 「更新的写」之后（否则磁盘会被回退到旧内容且永不重写）
                let (ws_id, files_saved, snapshot_revision) = {
                    let guard = current.read().unwrap_or_else(|e| e.into_inner());
                    let Some(ws) = guard.as_ref() else {
                        continue;
                    };
                    if !ws.is_dirty {
                        continue;
                    }

                    let ws_id = ws.id.clone();
                    // 修订号与快照同一读锁作用域内取得（否则「取快照 → 取修订号」
                    // 之间到来的改动会让修订号偏新，写盘的是旧内容却误判为可 clean）
                    let snapshot_revision = revision.load(Ordering::SeqCst);
                    let files = ws.files.clone();

                    let mut write_error = false;
                    for (file_name, content) in &files {
                        let path = std::path::PathBuf::from(file_name);
                        if let Err(e) = repo.save_file(&ws_id, &path, content) {
                            warn!(
                                workspace_id = ws_id,
                                file = file_name,
                                error = %e,
                                "自动保存失败"
                            );
                            write_error = true;
                        }
                    }

                    if write_error {
                        continue; // 保持脏：下一 tick 重试，且不发布「已落盘」
                    }

                    (ws_id, files.len(), snapshot_revision)
                };

                // 仅当快照之后没有新改动才标记 clean：否则「快照写盘」会被当成
                // 本次编辑已落盘，最新内容将停留在内存直到下一次编辑
                let persisted = {
                    let mut guard = current.write().unwrap_or_else(|e| e.into_inner());
                    match guard.as_mut() {
                        Some(ws)
                            if can_mark_clean(
                                &ws_id,
                                &ws.id,
                                snapshot_revision,
                                revision.load(Ordering::SeqCst),
                            ) =>
                        {
                            ws.mark_clean();
                            true
                        }
                        _ => false,
                    }
                };

                if persisted {
                    event_bus.publish(CoreEvent::WorkspaceSaved {
                        workspace_id: ws_id.clone(),
                        revision: snapshot_revision,
                        automatic: true,
                    });
                    debug!(
                        workspace_id = ws_id,
                        files_saved = files_saved,
                        "自动保存完成"
                    );
                } else {
                    debug!(
                        workspace_id = ws_id,
                        "自动保存期间有新改动，保留脏标记待下轮重写"
                    );
                }
            }
        });

        // 复用开头那把锁的 guard：`std::sync::Mutex` 不可重入，再次 lock 会自锁。
        *handle = Some(task);
        *self
            .auto_save_interval_secs
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(interval_secs);

        debug!(interval_secs = interval_secs, "自动保存已启动");
    }

    /// 停止自动保存。
    pub fn stop_auto_save(&self) {
        let mut handle = self
            .auto_save_handle
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(task) = handle.take() {
            task.abort();
            debug!("自动保存已停止");
        }
        *self
            .auto_save_interval_secs
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = None;
    }

    /// 当前 auto-save 的间隔（秒）；`None` = 未运行。
    ///
    /// 供命令层「按配置同步 auto-save」判断是否需要重启：间隔相同则保持不动，
    /// 免得每次 `load_workspace` 都重置计时器；配置关掉后也有状态可依，
    /// 再打开时能重新启动（旧实现用一次性 `static AtomicBool`，关掉后再也起不来）。
    #[must_use]
    pub fn auto_save_interval_secs(&self) -> Option<u64> {
        *self
            .auto_save_interval_secs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    /// 切换工作区：保存当前 → 加载目标。
    ///
    /// **不发布事件**：切换由前端经 IPC 发起，返回值即真值；切换前对旧工作区的
    /// 显式保存本身会发布 `WorkspaceSaved`（前端据此清脏，并按 workspace_id /
    /// revision 过滤掉不属于当前工作区的过期事件）。
    pub fn switch(&self, workspace_id: &str, root_path: &str) -> AppResult<Workspace> {
        // 保存当前工作区（内存是唯一权威副本，替换前必须先落盘）
        self.save_current_if_dirty();

        // 加载目标工作区
        let workspace = self.load(workspace_id, root_path)?;

        Ok(workspace)
    }

    /// 销毁工作区：删除所有文件并从内存清除。
    pub fn destroy(&self, workspace_id: &str) -> AppResult<()> {
        self.repo.delete_workspace(workspace_id)?;

        // 如果销毁的是当前工作区，则清空当前引用
        {
            let mut current = self.current.write().unwrap_or_else(|e| e.into_inner());
            let is_current = current.as_ref().is_some_and(|ws| ws.id == workspace_id);
            if is_current {
                *current = None;
            }
        }

        info!(workspace_id = workspace_id, "工作区已销毁");
        Ok(())
    }

    /// 崩溃恢复：启动时扫描所有已有的工作区文件并恢复。
    ///
    /// 当前实现为 stub —— WorkspaceRepository 没有 list_all_workspaces 方法。
    /// 恢复功能将在 Storage 层补充目录扫描能力后完善。
    pub fn recover_all(&self) -> AppResult<Vec<Workspace>> {
        debug!("崩溃恢复：当前阶段为 stub");
        // TODO: 需要 Storage::list_dirs() 或 WorkspaceRepository::list_workspaces()
        Ok(Vec::new())
    }

    // ── 文件操作 ──

    /// 更新当前工作区中的文件内容：**只写内存**，标记 dirty 并递增修订号。
    ///
    /// 落盘不是本方法的职责（与 `workspace_cmd::update_workspace_file` 的契约一致）：
    /// 前端 2 秒防抖把编辑器内容推进内存，磁盘写入由 auto-save 周期与显式
    /// [`Self::save`]（切题 / 失焦 / 关窗）负责。这样「自动保存间隔」才真正决定
    /// 落盘频率，也避免每个输入停顿都产生一次磁盘写。
    ///
    /// 修订号必须在写锁内递增：它与 auto-save 取快照的读锁构成全序，
    /// 是「快照写盘后能否标记 clean」的判据。
    pub fn update_file(&self, file_name: &str, content: &str) -> AppResult<()> {
        let mut current = self.current.write().unwrap_or_else(|e| e.into_inner());
        let ws = current
            .as_mut()
            .ok_or_else(|| AppError::Workspace("无当前工作区".into()))?;

        // 更新内存中的文件 + 标记 dirty。
        // 同时把该文件记为**当前代码文件**：写入路径的权威源由它承担，
        // 调用方不必再按语言派生文件名（派生会让语言切换后的写入落到别的文件上）。
        ws.files.insert(file_name.to_string(), content.to_string());
        ws.active_file = Some(file_name.to_string());
        ws.mark_dirty();
        self.revision.fetch_add(1, Ordering::SeqCst);

        debug!(
            workspace_id = ws.id,
            file = file_name,
            size = content.len(),
            "文件已更新（内存，等待 auto-save 落盘）"
        );

        // auto-save 负责落盘与发布事件，这里不重复发布
        Ok(())
    }

    /// 获取当前工作区中的文件内容。优先从内存读取，内存未命中时回退到磁盘。
    pub fn get_file(&self, file_name: &str) -> AppResult<String> {
        let current = self.current.read().unwrap_or_else(|e| e.into_inner());
        let ws = current
            .as_ref()
            .ok_or_else(|| AppError::Workspace("无当前工作区".into()))?;

        // 优先从内存返回
        if let Some(content) = ws.files.get(file_name) {
            return Ok(content.clone());
        }

        // 回退到磁盘
        self.repo
            .read_file(&ws.id, &PathBuf::from(file_name))
            .map_err(|e| {
                AppError::Workspace(format!(
                    "读取文件失败: workspace={}, file={}, 错误: {}",
                    ws.id, file_name, e
                ))
            })
    }

    /// 获取当前活动工作区。
    pub fn current(&self) -> Option<Workspace> {
        self.current.read().ok()?.clone()
    }

    /// 按 `contest_id` + `problem_id` 查找或创建工作区（修复 P36）。
    ///
    /// 扫描所有已有工作区的元数据，匹配 `contest_id` 和 `problem_id`：
    /// - 命中 → 加载已有工作区（恢复之前的代码）
    /// - 未命中 → 创建新工作区
    ///
    /// **不发布事件**：由前端经 IPC 发起，返回值即真值。
    pub fn find_or_create(
        &self,
        contest_id: &str,
        problem_id: &str,
        root_path: &str,
    ) -> AppResult<Workspace> {
        // 扫描已有工作区，查找匹配的 workspace.json 元数据
        let ids = self.repo.list_workspace_ids().unwrap_or_default();
        for ws_id in &ids {
            if let Ok(meta_json) = self.repo.read_file(ws_id, &PathBuf::from("workspace.json")) {
                if let Ok(meta) = serde_json::from_str::<WorkspaceMeta>(&meta_json) {
                    if meta.contest_id == contest_id && meta.problem_id == problem_id {
                        info!(
                            workspace_id = ws_id,
                            contest_id = contest_id,
                            problem_id = problem_id,
                            "找到已有工作区，恢复代码"
                        );
                        return self.load(ws_id, root_path);
                    }
                }
            }
        }

        // 未命中，创建新工作区
        debug!(
            contest_id = contest_id,
            problem_id = problem_id,
            "未找到已有工作区，创建新工作区"
        );
        self.create(contest_id, problem_id, root_path)
    }
}

impl Drop for WorkspaceManager {
    fn drop(&mut self) {
        self.stop_auto_save();
    }
}

#[cfg(test)]
#[path = "tests/manager_tests.rs"]
mod tests;
