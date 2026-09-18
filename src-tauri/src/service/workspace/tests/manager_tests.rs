use super::*;
use std::sync::Arc;

use crate::core::event::event_bus::EventBus;
use crate::infra::fs_workspace_repo::FsWorkspaceRepository;
use crate::infra::storage::Storage;

fn test_manager(test_name: &str) -> WorkspaceManager {
    let dir = std::env::temp_dir().join(format!("hinina-test-mgr-{}", test_name));
    let _ = std::fs::remove_dir_all(&dir);
    let storage = Arc::new(Storage::new(dir));
    let repo = Arc::new(FsWorkspaceRepository::new(storage));
    let event_bus = Arc::new(EventBus::new());
    WorkspaceManager::new(repo, event_bus)
}

fn test_manager_with_storage(test_name: &str) -> (WorkspaceManager, Arc<Storage>) {
    let dir = std::env::temp_dir().join(format!("hinina-test-mgr-{}", test_name));
    let _ = std::fs::remove_dir_all(&dir);
    let storage = Arc::new(Storage::new(dir));
    let repo = Arc::new(FsWorkspaceRepository::new(Arc::clone(&storage)));
    let event_bus = Arc::new(EventBus::new());
    (WorkspaceManager::new(repo, event_bus), storage)
}

/// 同上，额外把 EventBus 交给调用方（auto-save 事件断言需要订阅它）。
fn test_manager_with_bus(test_name: &str) -> (WorkspaceManager, Arc<Storage>, Arc<EventBus>) {
    let dir = std::env::temp_dir().join(format!("hinina-test-mgr-{}", test_name));
    let _ = std::fs::remove_dir_all(&dir);
    let storage = Arc::new(Storage::new(dir));
    let repo = Arc::new(FsWorkspaceRepository::new(Arc::clone(&storage)));
    let event_bus = Arc::new(EventBus::new());
    (
        WorkspaceManager::new(repo, Arc::clone(&event_bus)),
        storage,
        event_bus,
    )
}

#[test]
fn create_sets_current_workspace() {
    let mgr = test_manager("create-current");
    let ws = mgr
        .create("contest-1", "problem-A", "/home/user/ws")
        .unwrap();

    assert_eq!(ws.contest_id, "contest-1");
    assert_eq!(ws.problem_id, "problem-A");
    assert!(!ws.files.contains_key("workspace.json"));

    let current = mgr.current().unwrap();
    assert_eq!(current.id, ws.id);
    assert_eq!(current.contest_id, "contest-1");
    assert_eq!(current.problem_id, "problem-A");
}

#[test]
fn update_file_defers_disk_write_until_save() {
    // debounce-to-memory 语义：前端 2 秒防抖只推内存，落盘由 save / auto-save 负责。
    // 若这里改回写透，则「自动保存间隔」不再决定落盘频率（每个输入停顿都写盘）。
    let (mgr, storage) = test_manager_with_storage("update-memory-only");
    mgr.create("contest-2", "problem-B", "/ws").unwrap();
    mgr.update_file("main.cpp", "int main() { return 0; }")
        .unwrap();

    let repo = FsWorkspaceRepository::new(Arc::clone(&storage));
    let ws = mgr.current().unwrap();
    assert!(
        repo.read_file(&ws.id, &std::path::PathBuf::from("main.cpp"))
            .is_err(),
        "update_file 不应落盘"
    );
    assert!(ws.is_dirty, "推送到内存后工作区应为脏");
    // 内存可见（get_file 优先读内存）
    assert_eq!(
        mgr.get_file("main.cpp").unwrap(),
        "int main() { return 0; }"
    );

    mgr.save().unwrap();

    assert_eq!(
        repo.read_file(&ws.id, &std::path::PathBuf::from("main.cpp"))
            .unwrap(),
        "int main() { return 0; }",
        "save 必须落盘"
    );
}

#[test]
fn create_saves_previous_workspace_before_replacing() {
    // 内存是唯一权威副本：create 替换 current 前必须落盘旧工作区，
    // 否则未落盘的改动随替换静默消失
    let (mgr, storage) = test_manager_with_storage("create-saves-previous");
    let first = mgr.create("contest-2", "problem-B", "/ws").unwrap();
    mgr.update_file("main.cpp", "// first").unwrap();

    mgr.create("contest-3", "problem-C", "/ws").unwrap();

    let repo = FsWorkspaceRepository::new(Arc::clone(&storage));
    assert_eq!(
        repo.read_file(&first.id, &std::path::PathBuf::from("main.cpp"))
            .unwrap(),
        "// first",
        "替换当前工作区前必须落盘旧的"
    );
}

#[test]
fn load_saves_previous_workspace_before_replacing() {
    // 同一工作区重载（切走再切回）时：先落盘再读回，保证内存里的最新内容不被旧磁盘内容覆盖
    let (mgr, storage) = test_manager_with_storage("load-saves-previous");
    let ws = mgr.create("contest-2", "problem-B", "/ws").unwrap();
    mgr.update_file("main.cpp", "// latest").unwrap();

    let reloaded = mgr.load(&ws.id, "/ws").unwrap();

    assert_eq!(
        reloaded.files.get("main.cpp").map(String::as_str),
        Some("// latest"),
        "重载不得回退到旧磁盘内容"
    );
    assert!(!reloaded.is_dirty, "重载后内存与磁盘一致，应为干净");

    // 且内存内容确已落盘（不是只留在内存里）
    let repo = FsWorkspaceRepository::new(Arc::clone(&storage));
    assert_eq!(
        repo.read_file(&ws.id, &std::path::PathBuf::from("main.cpp"))
            .unwrap(),
        "// latest"
    );
}

#[test]
fn save_marks_workspace_clean() {
    let mgr = test_manager("save-clean");
    mgr.create("contest-3", "problem-C", "/ws").unwrap();
    mgr.update_file("main.cpp", "#include <cstdio>")
        .unwrap();

    let ws = mgr.current().unwrap();
    assert!(ws.is_dirty, "workspace should be dirty after update_file");

    mgr.save().unwrap();

    let ws = mgr.current().unwrap();
    assert!(!ws.is_dirty, "workspace should be clean after save");
}

#[test]
fn update_file_adds_to_files_and_marks_dirty() {
    let mgr = test_manager("update-dirty");
    mgr.create("contest-4", "problem-D", "/ws").unwrap();
    mgr.update_file("solution.py", "print(42)").unwrap();

    let ws = mgr.current().unwrap();
    assert!(ws.files.contains_key("solution.py"));
    assert_eq!(ws.files["solution.py"], "print(42)");
    assert!(ws.is_dirty, "workspace should be dirty after update_file");
}

#[test]
fn get_file_reads_from_memory() {
    let mgr = test_manager("get-file-mem");
    mgr.create("contest-5", "problem-E", "/ws").unwrap();
    let code = "#include <stdio.h>\nint main() { return 0; }";
    mgr.update_file("main.c", code).unwrap();

    let content = mgr.get_file("main.c").unwrap();
    assert_eq!(content, code);
}

#[test]
fn load_recovers_workspace_from_disk() {
    let dir = std::env::temp_dir().join("hinina-test-mgr-load-recover");
    let _ = std::fs::remove_dir_all(&dir);
    let storage = Arc::new(Storage::new(dir));
    let repo1 = Arc::new(FsWorkspaceRepository::new(Arc::clone(&storage)));
    let mgr1 = WorkspaceManager::new(repo1, Arc::new(EventBus::new()));

    let ws = mgr1
        .create("contest-6", "problem-F", "/home/user/oj")
        .unwrap();
    let ws_id = ws.id.clone();
    mgr1.update_file("main.cpp", "// recovered file").unwrap();
    mgr1.save().unwrap(); // 内存是唯一权威副本：跨实例恢复前必须落盘

    // create a second manager sharing the same storage
    let repo2 = Arc::new(FsWorkspaceRepository::new(storage));
    let mgr2 = WorkspaceManager::new(repo2, Arc::new(EventBus::new()));

    let loaded = mgr2.load(&ws_id, "/home/user/oj").unwrap();
    assert_eq!(loaded.contest_id, "contest-6");
    assert_eq!(loaded.problem_id, "problem-F");
    assert_eq!(
        loaded.files.get("main.cpp").unwrap(),
        "// recovered file"
    );
}

#[test]
fn load_uses_metadata_not_id_parsing() {
    let dir = std::env::temp_dir().join("hinina-test-mgr-metadata");
    let _ = std::fs::remove_dir_all(&dir);
    let storage = Arc::new(Storage::new(dir));
    let repo1 = Arc::new(FsWorkspaceRepository::new(Arc::clone(&storage)));
    let mgr1 = WorkspaceManager::new(repo1, Arc::new(EventBus::new()));

    let ws = mgr1
        .create("contest-2024", "problem-G", "/ws")
        .unwrap();
    let ws_id = ws.id.clone();
    mgr1.update_file("main.cpp", "// 2024 contest").unwrap();
    mgr1.save().unwrap(); // 内存是唯一权威副本：跨实例恢复前必须落盘

    let repo2 = Arc::new(FsWorkspaceRepository::new(storage));
    let mgr2 = WorkspaceManager::new(repo2, Arc::new(EventBus::new()));

    let loaded = mgr2.load(&ws_id, "/ws").unwrap();

    // If contest_id were parsed from workspace ID (naively splitting on '-'),
    // it would be "contest" instead of "contest-2024".
    // The load method uses workspace.json metadata, preserving the full value.
    assert_eq!(
        loaded.contest_id, "contest-2024",
        "contest_id should come from metadata, not ID parsing"
    );
    assert_eq!(loaded.problem_id, "problem-G");
}

#[test]
fn destroy_removes_workspace() {
    let (mgr, storage) = test_manager_with_storage("destroy");
    let ws = mgr
        .create("contest-7", "problem-H", "/ws")
        .unwrap();
    let ws_id = ws.id.clone();

    let repo = FsWorkspaceRepository::new(Arc::clone(&storage));
    assert!(repo.exists(&ws_id), "workspace should exist before destroy");

    mgr.destroy(&ws_id).unwrap();
    assert!(!repo.exists(&ws_id), "workspace should not exist after destroy");
}

#[test]
fn switch_saves_current_and_loads_target() {
    let mgr = test_manager("switch");

    let ws1 = mgr
        .create("contest-8", "problem-I", "/ws")
        .unwrap();
    let ws1_id = ws1.id.clone();
    mgr.update_file("main.cpp", "// workspace one").unwrap();

    mgr.create("contest-9", "problem-J", "/ws").unwrap();

    // switch back to ws1 — should preserve its file
    mgr.switch(&ws1_id, "/ws").unwrap();

    let content = mgr.get_file("main.cpp").unwrap();
    assert_eq!(content, "// workspace one");
}

#[test]
fn current_returns_none_when_no_workspace() {
    let mgr = test_manager("current-none");
    assert!(mgr.current().is_none());

    let ws = mgr
        .create("contest-10", "problem-K", "/ws")
        .unwrap();
    assert!(mgr.current().is_some());

    mgr.destroy(&ws.id).unwrap();
    assert!(mgr.current().is_none());
}

// ── 语言持久化 ──
//
// 语言不属于任何代码文件：若保存路径只写文件，切换语言后切题或重启会退回默认语言，
// 选手的 Java 代码会被当作 C++ 提交 —— 赛场上最难排查的静默故障。

#[test]
fn set_language_persists_across_manager_instances() {
    let (mgr, storage) = test_manager_with_storage("set-language");
    let ws = mgr
        .create("contest-1", "problem-A", "/ws")
        .expect("创建工作区失败");
    assert_eq!(ws.language, "", "新建工作区语言为空");

    let updated = mgr.set_language("java").expect("设置语言失败");
    assert_eq!(updated.language, "java");
    assert!(!updated.is_dirty, "语言变更不应把代码文件标记为脏");

    // 换一个 manager 实例，模拟切题后重新加载 / 客户端重启
    let repo = Arc::new(FsWorkspaceRepository::new(Arc::clone(&storage)));
    let mgr2 = WorkspaceManager::new(repo, Arc::new(EventBus::new()));
    let loaded = mgr2.load(&ws.id, "/ws").expect("加载工作区失败");
    assert_eq!(loaded.language, "java", "语言必须跨实例持久化");
}

#[test]
fn save_persists_language_metadata() {
    let (mgr, storage) = test_manager_with_storage("save-language");
    let ws = mgr
        .create("contest-1", "problem-A", "/ws")
        .expect("创建工作区失败");
    mgr.update_file("Main.java", "class Main {}").expect("写入文件失败");
    mgr.set_language("java").expect("设置语言失败");
    mgr.save().expect("保存失败");

    let repo = Arc::new(FsWorkspaceRepository::new(Arc::clone(&storage)));
    let mgr2 = WorkspaceManager::new(repo, Arc::new(EventBus::new()));
    let loaded = mgr2.load(&ws.id, "/ws").expect("加载工作区失败");
    assert_eq!(loaded.language, "java");
    assert_eq!(
        loaded.files.get("Main.java").map(String::as_str),
        Some("class Main {}")
    );
}

#[test]
fn set_language_without_current_workspace_errors() {
    let mgr = test_manager("set-language-no-ws");
    let err = mgr.set_language("cpp").expect_err("无当前工作区时应报错");
    assert!(matches!(err, AppError::Workspace(_)));
}

// ── auto-save：落盘语义、修订号防线与事件契约 ──
//
// auto-save 是 debounce-to-memory 语义下唯一的周期落盘路径，且它的事件会被
// main.rs 桥接为前端「已自动备份」指示 —— 事件必须只在「最新内容确已落盘」时发出。

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

use crate::core::event::app_event::{AppEvent, WorkspaceEvent};
use crate::core::event::event_category::EventCategory;
use crate::core::repository::workspace_repo::WorkspaceRepository;

/// 写盘钩子：在真实写盘前执行（返回 Err 即模拟写失败）
type SaveHook = Arc<dyn Fn(&str, &str) -> AppResult<()> + Send + Sync>;

/// 可注入副作用的仓库桩：在真实写盘前执行钩子。
///
/// 用于确定性复现两种时序：写盘期间编辑器又推了新内容、写盘失败。
struct HookedRepo {
    inner: Arc<FsWorkspaceRepository>,
    on_save: SaveHook,
}

impl WorkspaceRepository for HookedRepo {
    fn save_file(&self, workspace_id: &str, path: &Path, content: &str) -> AppResult<()> {
        (self.on_save)(workspace_id, content)?;
        self.inner.save_file(workspace_id, path, content)
    }

    fn read_file(&self, workspace_id: &str, path: &Path) -> AppResult<String> {
        self.inner.read_file(workspace_id, path)
    }

    fn list_files(&self, workspace_id: &str) -> AppResult<Vec<std::path::PathBuf>> {
        self.inner.list_files(workspace_id)
    }

    fn delete_workspace(&self, workspace_id: &str) -> AppResult<()> {
        self.inner.delete_workspace(workspace_id)
    }

    fn exists(&self, workspace_id: &str) -> bool {
        self.inner.exists(workspace_id)
    }

    fn list_workspace_ids(&self) -> AppResult<Vec<String>> {
        self.inner.list_workspace_ids()
    }
}

/// 订阅 `AutoSaveTriggered` 并返回计数句柄。
fn count_auto_save_events(bus: &Arc<EventBus>) -> Arc<AtomicUsize> {
    let count = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&count);
    bus.subscribe(
        EventCategory::Workspace,
        Arc::new(move |event: &AppEvent| {
            if matches!(
                event,
                AppEvent::Workspace(WorkspaceEvent::AutoSaveTriggered { .. })
            ) {
                counter.fetch_add(1, AtomicOrdering::SeqCst);
            }
        }),
    );
    count
}

#[tokio::test]
async fn auto_save_persists_dirty_workspace_and_publishes_once() {
    let (mgr, storage, bus) = test_manager_with_bus("autosave-persist");
    let events = count_auto_save_events(&bus);

    let ws = mgr.create("c", "p", "/ws").unwrap();
    mgr.update_file("main.cpp", "v1").unwrap();
    assert!(mgr.current().unwrap().is_dirty);

    mgr.start_auto_save(1);
    tokio::time::sleep(std::time::Duration::from_millis(1_600)).await;
    mgr.stop_auto_save();

    assert!(!mgr.current().unwrap().is_dirty, "落盘后应标记干净");
    assert_eq!(events.load(AtomicOrdering::SeqCst), 1, "应发布一次落盘事件");
    let repo = FsWorkspaceRepository::new(Arc::clone(&storage));
    assert_eq!(
        repo.read_file(&ws.id, &std::path::PathBuf::from("main.cpp"))
            .unwrap(),
        "v1"
    );

    let _ = std::fs::remove_dir_all(std::env::temp_dir().join("hinina-test-mgr-autosave-persist"));
}

#[tokio::test]
async fn auto_save_keeps_dirty_when_newer_edit_arrives_during_write() {
    let dir = std::env::temp_dir().join("hinina-test-mgr-autosave-race");
    let _ = std::fs::remove_dir_all(&dir);
    let storage = Arc::new(Storage::new(dir.clone()));
    let real_repo = Arc::new(FsWorkspaceRepository::new(Arc::clone(&storage)));
    let bus = Arc::new(EventBus::new());
    let events = count_auto_save_events(&bus);

    // 钩子：写 "v1" 期间编辑器推入 "v2"（模拟快照之后到来的新改动）
    let slot: Arc<std::sync::Mutex<Option<Arc<WorkspaceManager>>>> =
        Arc::new(std::sync::Mutex::new(None));
    let hook_slot = Arc::clone(&slot);
    let on_save: SaveHook = Arc::new(move |_ws_id, content| {
        if content == "v1" {
            if let Some(mgr) = hook_slot.lock().unwrap().clone() {
                mgr.update_file("main.cpp", "v2").unwrap();
            }
        }
        Ok(())
    });

    let repo: Arc<dyn WorkspaceRepository> = Arc::new(HookedRepo {
        inner: Arc::clone(&real_repo),
        on_save,
    });
    let mgr = Arc::new(WorkspaceManager::new(repo, Arc::clone(&bus)));
    *slot.lock().unwrap() = Some(Arc::clone(&mgr));

    mgr.create("c", "p", "/ws").unwrap();
    mgr.update_file("main.cpp", "v1").unwrap();

    mgr.start_auto_save(1);
    // 第一轮：写入的是快照 "v1"，但期间修订号已变 —— 不得标记干净、不得发布事件
    tokio::time::sleep(std::time::Duration::from_millis(1_600)).await;
    assert!(
        mgr.current().unwrap().is_dirty,
        "快照之后有新改动时必须保留脏标记（否则新内容永远不会落盘）"
    );
    assert_eq!(
        events.load(AtomicOrdering::SeqCst),
        0,
        "内容未确证落盘时不得发布「已落盘」事件"
    );
    assert_eq!(mgr.get_file("main.cpp").unwrap(), "v2", "内存应保留最新内容");

    // 第二轮：修订号未再变化，正常落盘并发布事件
    tokio::time::sleep(std::time::Duration::from_millis(1_500)).await;
    mgr.stop_auto_save();
    assert!(!mgr.current().unwrap().is_dirty, "第二轮应落盘并标记干净");
    assert_eq!(events.load(AtomicOrdering::SeqCst), 1);
    assert_eq!(
        real_repo
            .read_file(
                &mgr.current().unwrap().id,
                &std::path::PathBuf::from("main.cpp")
            )
            .unwrap(),
        "v2",
        "最新内容最终必须落盘"
    );

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn auto_save_keeps_dirty_and_silent_when_write_fails() {
    let dir = std::env::temp_dir().join("hinina-test-mgr-autosave-fail");
    let _ = std::fs::remove_dir_all(&dir);
    let storage = Arc::new(Storage::new(dir.clone()));
    let real_repo = Arc::new(FsWorkspaceRepository::new(Arc::clone(&storage)));
    let bus = Arc::new(EventBus::new());
    let events = count_auto_save_events(&bus);

    let on_save: SaveHook = Arc::new(|_id, content| {
        if content == "v1" {
            return Err(AppError::Io("模拟磁盘写失败".into()));
        }
        Ok(())
    });
    let repo: Arc<dyn WorkspaceRepository> = Arc::new(HookedRepo {
        inner: real_repo,
        on_save,
    });
    let mgr = WorkspaceManager::new(repo, Arc::clone(&bus));

    mgr.create("c", "p", "/ws").unwrap();
    mgr.update_file("main.cpp", "v1").unwrap();

    mgr.start_auto_save(1);
    tokio::time::sleep(std::time::Duration::from_millis(1_600)).await;
    mgr.stop_auto_save();

    assert!(
        mgr.current().unwrap().is_dirty,
        "写失败必须保留脏标记，等待下轮重试"
    );
    assert_eq!(
        events.load(AtomicOrdering::SeqCst),
        0,
        "写失败不得发布「已落盘」事件（否则前端会显示已自动备份）"
    );

    let _ = std::fs::remove_dir_all(dir);
}
