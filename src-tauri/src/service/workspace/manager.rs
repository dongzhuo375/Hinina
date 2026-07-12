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
    event_bus: Arc<EventBus>,
    /// 当前活动工作区（RwLock 内直接持有 Workspace，支持 auto-save 共享和可变访问）
    current: Arc<RwLock<Option<Workspace>>>,
    /// 自动保存取消标记
    auto_save_running: AtomicBool,
    /// 自动保存的 JoinHandle
    auto_save_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

/// 工作区元数据，持久化在 workspace.json 中，避免从 workspace_id 字符串解析字段。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkspaceMeta {
    contest_id: String,
    problem_id: String,
    root_path: String,
    language: String,
    created_at: i64,
    updated_at: i64,
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
    ) -> AppResult<Workspace> {
        let ws = Workspace::new(
            contest_id.to_string(),
            problem_id.to_string(),
            root_path.to_string(),
        );

        // 持久化元数据到 workspace.json，避免从 workspace_id 字符串解析元数据
        let meta = WorkspaceMeta {
            contest_id: contest_id.to_string(),
            problem_id: problem_id.to_string(),
            root_path: root_path.to_string(),
            language: String::new(),
            created_at: ws.created_at,
            updated_at: ws.updated_at,
        };
        let meta_json = serde_json::to_string_pretty(&meta)
            .map_err(|e| AppError::Serialization(format!("序列化工作区元数据失败: {}", e)))?;
        self.repo.save_file(&ws.id, &PathBuf::from("workspace.json"), &meta_json)?;

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

        self.event_bus
            .publish(&AppEvent::Workspace(WorkspaceEvent::Loaded {
                workspace_id: ws.id.clone(),
            }));

        Ok(ws)
    }

    /// 加载已有工作区：从磁盘恢复所有文件到内存。
    ///
    /// 发布 `WorkspaceEvent::Loaded`。
    pub fn load(&self, workspace_id: &str, _root_path: &str) -> AppResult<Workspace> {
        if !self.repo.exists(workspace_id) {
            return Err(AppError::Workspace(format!(
                "工作区不存在: {}",
                workspace_id
            )));
        }

        let file_paths = self.repo.list_files(workspace_id)?;

        // 读取元数据
        let meta_json = self.repo.read_file(workspace_id, &PathBuf::from("workspace.json"))?;
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

        self.event_bus
            .publish(&AppEvent::Workspace(WorkspaceEvent::Loaded {
                workspace_id: workspace_id.to_string(),
            }));

        Ok(ws)
    }

    /// 保存当前工作区的所有脏文件到磁盘。
    ///
    /// 发布 `WorkspaceEvent::Saved`。
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

        ws.mark_clean();

        debug!(
            workspace_id = ws.id,
            files_saved = saved,
            "工作区已保存"
        );

        let ws_id = ws.id.clone();
        drop(current);

        self.event_bus
            .publish(&AppEvent::Workspace(WorkspaceEvent::Saved {
                workspace_id: ws_id,
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

                let guard = current.read().unwrap_or_else(|e| e.into_inner());

                if let Some(ref ws) = *guard {
                    if ws.is_dirty {
                        // 克隆文件集，在释放锁后异步写入
                        let files: HashMap<String, String> = ws.files.clone();
                        let ws_id = ws.id.clone();
                        drop(guard);

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

                        // 写入成功后标记 clean，避免下一 tick 重复写入
                        if !write_error {
                            let mut guard = current
                                .write()
                                .unwrap_or_else(|e| e.into_inner());
                            // 仅在仍是同一个工作区时重置 dirty 标记
                            if let Some(ref mut ws) = *guard {
                                if ws.id == ws_id {
                                    ws.mark_clean();
                                }
                            }
                        }

                        event_bus.publish(&AppEvent::Workspace(
                            WorkspaceEvent::AutoSaveTriggered {
                                workspace_id: ws_id.clone(),
                            },
                        ));
                        debug!(workspace_id = ws_id, "自动保存完成");
                    } else {
                        drop(guard);
                    }
                } else {
                    drop(guard);
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
    pub fn switch(&self, workspace_id: &str, root_path: &str) -> AppResult<Workspace> {
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

    /// 更新当前工作区中的文件内容，标记为 dirty 并持久化到磁盘。
    pub fn update_file(&self, file_name: &str, content: &str) -> AppResult<()> {
        let mut current = self.current.write().unwrap_or_else(|e| e.into_inner());
        let ws = current
            .as_mut()
            .ok_or_else(|| AppError::Workspace("无当前工作区".into()))?;

        // 更新内存中的文件 + 标记 dirty
        ws.files.insert(file_name.to_string(), content.to_string());
        ws.mark_dirty();

        // 持久化到磁盘
        self.repo
            .save_file(&ws.id, &PathBuf::from(file_name), content)?;

        debug!(
            workspace_id = ws.id,
            file = file_name,
            size = content.len(),
            "文件已更新"
        );

        // auto-save 负责发布事件，这里不重复发布
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
}

impl Drop for WorkspaceManager {
    fn drop(&mut self) {
        self.stop_auto_save();
    }
}

#[cfg(test)]
#[path = "tests/manager_tests.rs"]
mod tests;
