use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use tracing::{debug, info, warn};

use crate::core::entity::workspace::Workspace;
use crate::core::error::{AppError, AppResult};
use crate::core::event::app_event::{AppEvent, WorkspaceEvent};
use crate::core::event::event_bus::EventBus;
use crate::core::repository::workspace_repo::WorkspaceRepository;

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
    event_bus: Arc<EventBus>,
    /// 当前活动工作区（Arc 包装以支持 auto-save 任务安全共享）
    current: Arc<RwLock<Option<Arc<Workspace>>>>,
    /// 自动保存取消标记
    auto_save_running: AtomicBool,
    /// 自动保存的 JoinHandle
    auto_save_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl WorkspaceManager {
    /// 创建 WorkspaceManager。
    pub fn new(repo: Arc<dyn WorkspaceRepository>, event_bus: Arc<EventBus>) -> Self {
        Self {
            repo,
            event_bus,
            current: Arc::new(RwLock::new(None)),
            auto_save_running: AtomicBool::new(false),
            auto_save_handle: Mutex::new(None),
        }
    }

    // ── 生命周期方法 ──

    /// 创建新工作区，初始化空文件集合并设为当前。
    ///
    /// 发布 `WorkspaceEvent::Loaded`。
    pub fn create(
        &self,
        contest_id: &str,
        problem_id: &str,
        root_path: &str,
    ) -> AppResult<Arc<Workspace>> {
        let workspace = Arc::new(Workspace::new(
            contest_id.to_string(),
            problem_id.to_string(),
            root_path.to_string(),
        ));

        {
            let mut current = self.current.write().unwrap_or_else(|e| e.into_inner());
            *current = Some(Arc::clone(&workspace));
        }

        info!(
            workspace_id = workspace.id,
            contest_id = contest_id,
            problem_id = problem_id,
            "工作区已创建"
        );

        self.event_bus
            .publish(&AppEvent::Workspace(WorkspaceEvent::Loaded {
                workspace_id: workspace.id.clone(),
            }));

        Ok(workspace)
    }

    /// 加载已有工作区：从磁盘恢复所有文件到内存。
    ///
    /// 发布 `WorkspaceEvent::Loaded`。
    pub fn load(&self, workspace_id: &str, root_path: &str) -> AppResult<Arc<Workspace>> {
        if !self.repo.exists(workspace_id) {
            return Err(AppError::Workspace(format!(
                "工作区不存在: {}",
                workspace_id
            )));
        }

        let file_paths = self.repo.list_files(workspace_id)?;
        let mut files = HashMap::new();

        for path in &file_paths {
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

        // 从 workspace_id 解析 contest_id 和 problem_id
        // ID 格式: ws-{contest_id}-{problem_id}-{timestamp}-{hex}
        let parts: Vec<&str> = workspace_id.split('-').collect();
        let contest_id = parts.get(1).map(|s| s.to_string()).unwrap_or_default();
        let problem_id = parts.get(2).map(|s| s.to_string()).unwrap_or_default();

        let workspace = Arc::new(Workspace {
            id: workspace_id.to_string(),
            contest_id,
            problem_id,
            root_path: root_path.to_string(),
            files,
            language: String::new(),
            is_dirty: false,
            created_at: 0, // 恢复时不保留精确创建时间
            updated_at: 0,
        });

        {
            let mut current = self.current.write().unwrap_or_else(|e| e.into_inner());
            *current = Some(Arc::clone(&workspace));
        }

        info!(workspace_id = workspace_id, "工作区已加载");

        self.event_bus
            .publish(&AppEvent::Workspace(WorkspaceEvent::Loaded {
                workspace_id: workspace_id.to_string(),
            }));

        Ok(workspace)
    }

    /// 保存当前工作区的所有脏文件到磁盘。
    ///
    /// 发布 `WorkspaceEvent::Saved`。
    pub fn save(&self) -> AppResult<()> {
        let current = self
            .current
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        let ws = match current {
            Some(ref ws) => ws,
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

        // 通过 Arc 内部可变性标记为 clean
        // 注意：Workspace 需要内部可变性支持
        // 当前设计：Workspace 不可变，save 后通过创建新对象替换
        // 暂时跳过 mark_clean 调用（Workspace 是 Arc 包装的不可变对象）

        debug!(
            workspace_id = ws.id,
            files_saved = saved,
            "工作区已保存"
        );

        self.event_bus
            .publish(&AppEvent::Workspace(WorkspaceEvent::Saved {
                workspace_id: ws.id.clone(),
            }));

        Ok(())
    }

    /// 启动后台自动保存。
    ///
    /// 如果已有自动保存任务运行，则先停止旧的再启动。
    pub fn start_auto_save(&self, interval_secs: u64) {
        self.stop_auto_save();

        self.auto_save_running.store(true, Ordering::SeqCst);

        // auto-save 通过 Arc 共享 current 状态，安全且 Send。
        let repo = Arc::clone(&self.repo);
        let event_bus = Arc::clone(&self.event_bus);
        let current = Arc::clone(&self.current);

        let task = tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(Duration::from_secs(interval_secs));
            // 跳过首次立即触发
            interval_timer.tick().await;

            loop {
                interval_timer.tick().await;

                let ws_opt = current
                    .read()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();

                if let Some(ref ws) = ws_opt {
                    for (file_name, content) in &ws.files {
                        let path = std::path::PathBuf::from(file_name);
                        if let Err(e) = repo.save_file(&ws.id, &path, content) {
                            warn!(
                                workspace_id = ws.id,
                                file = file_name,
                                error = %e,
                                "自动保存失败"
                            );
                        }
                    }
                    event_bus.publish(&AppEvent::Workspace(
                        WorkspaceEvent::AutoSaveTriggered {
                            workspace_id: ws.id.clone(),
                        },
                    ));
                    debug!(workspace_id = ws.id, "自动保存完成");
                }
            }
        });

        let mut handle = self.auto_save_handle.lock().unwrap_or_else(|e| e.into_inner());
        *handle = Some(task);

        debug!(interval_secs = interval_secs, "自动保存已启动");
    }

    /// 停止自动保存。
    pub fn stop_auto_save(&self) {
        self.auto_save_running.store(false, Ordering::SeqCst);
        let mut handle = self.auto_save_handle.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(task) = handle.take() {
            task.abort();
            debug!("自动保存已停止");
        }
    }

    /// 切换工作区：保存当前 → 加载目标。
    ///
    /// 发布 `WorkspaceEvent::Switched`。
    pub fn switch(&self, workspace_id: &str, root_path: &str) -> AppResult<Arc<Workspace>> {
        let from = {
            let current = self.current.read().unwrap_or_else(|e| e.into_inner());
            current.as_ref().map(|ws| ws.id.clone())
        };

        // 保存当前工作区
        if let Err(e) = self.save() {
            warn!(error = %e, "切换前保存失败");
        }

        // 加载目标工作区
        let workspace = self.load(workspace_id, root_path)?;

        self.event_bus
            .publish(&AppEvent::Workspace(WorkspaceEvent::Switched {
                from: from.unwrap_or_default(),
                to: workspace_id.to_string(),
            }));

        Ok(workspace)
    }

    /// 销毁工作区：删除所有文件并从内存清除。
    pub fn destroy(&self, workspace_id: &str) -> AppResult<()> {
        self.repo.delete_workspace(workspace_id)?;

        // 如果销毁的是当前工作区，则清空当前引用
        {
            let mut current = self.current.write().unwrap_or_else(|e| e.into_inner());
            if let Some(ref ws) = *current {
                if ws.id == workspace_id {
                    *current = None;
                }
            }
        }

        info!(workspace_id = workspace_id, "工作区已销毁");
        Ok(())
    }

    /// 崩溃恢复：启动时扫描所有已有的工作区文件并恢复。
    ///
    /// 当前实现为 stub —— WorkspaceRepository 没有 list_all_workspaces 方法。
    /// 恢复功能将在 Storage 层补充目录扫描能力后完善。
    pub fn recover_all(&self) -> AppResult<Vec<Arc<Workspace>>> {
        debug!("崩溃恢复：当前阶段为 stub");
        // TODO: 需要 Storage::list_dirs() 或 WorkspaceRepository::list_workspaces()
        Ok(Vec::new())
    }

    // ── 文件操作 ──

    /// 更新当前工作区中的文件内容，标记为 dirty。
    pub fn update_file(&self, file_name: &str, content: &str) -> AppResult<()> {
        let current = self
            .current
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        let ws = match current {
            Some(ref ws) => ws,
            None => {
                return Err(AppError::Workspace("无当前工作区".into()));
            }
        };

        // Workspace 是不可变的 Arc，需要通过创建新对象来更新
        // 简化方案：直接操作 files HashMap 并标记 dirty
        // 这需要 Workspace 内部使用 Mutex 或 RefCell
        // 当前为简化的 unsafe 访问模式，下一版本改为 RwLock<Workspace>

        // 直接保存文件到 repo
        self.repo
            .save_file(&ws.id, &PathBuf::from(file_name), content)?;

        debug!(
            workspace_id = ws.id,
            file = file_name,
            size = content.len(),
            "文件已更新"
        );

        self.event_bus
            .publish(&AppEvent::Workspace(WorkspaceEvent::AutoSaveTriggered {
                workspace_id: ws.id.clone(),
            }));

        Ok(())
    }

    /// 获取当前工作区中的文件内容。
    pub fn get_file(&self, file_name: &str) -> AppResult<String> {
        let current = self
            .current
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        let ws = match current {
            Some(ref ws) => ws,
            None => {
                return Err(AppError::Workspace("无当前工作区".into()));
            }
        };

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
    pub fn current(&self) -> Option<Arc<Workspace>> {
        self.current.read().ok()?.clone()
    }
}

impl Drop for WorkspaceManager {
    fn drop(&mut self) {
        self.stop_auto_save();
    }
}
