use super::*;
use std::sync::Arc;

use crate::core::event::core_event_bus::CoreEventBus;
use crate::infra::fs_workspace_repo::FsWorkspaceRepository;
use crate::infra::storage::Storage;
use crate::test_support::{Guarded, TempDir};

/// 测试用管理器 + 临时目录守卫。
///
/// `Guarded` 把目录生命周期绑到管理器上：调用点仍按 `WorkspaceManager` 使用
/// （`Deref` 转发），目录在用例结束时随 `Drop` 回收。
fn test_manager(test_name: &str) -> Guarded<WorkspaceManager> {
    let dir = TempDir::named(&format!("hinina-test-mgr-{}", test_name));
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let repo = Arc::new(FsWorkspaceRepository::new(storage));
    let event_bus = Arc::new(CoreEventBus::new());
    Guarded::new(WorkspaceManager::new(repo, event_bus), dir)
}

fn test_manager_with_storage(test_name: &str) -> (Guarded<WorkspaceManager>, Arc<Storage>) {
    let dir = TempDir::named(&format!("hinina-test-mgr-{}", test_name));
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let repo = Arc::new(FsWorkspaceRepository::new(Arc::clone(&storage)));
    let event_bus = Arc::new(CoreEventBus::new());
    (
        Guarded::new(WorkspaceManager::new(repo, event_bus), dir),
        storage,
    )
}

/// 同上，额外把事件总线交给调用方（auto-save / 落盘事件断言需要订阅它）。
fn test_manager_with_bus(
    test_name: &str,
) -> (Guarded<WorkspaceManager>, Arc<Storage>, Arc<CoreEventBus>) {
    let dir = TempDir::named(&format!("hinina-test-mgr-{}", test_name));
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let repo = Arc::new(FsWorkspaceRepository::new(Arc::clone(&storage)));
    let event_bus = Arc::new(CoreEventBus::new());
    (
        Guarded::new(WorkspaceManager::new(repo, Arc::clone(&event_bus)), dir),
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
    mgr.update_file("main.cpp", "#include <cstdio>").unwrap();

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

// ── delete_file（P62：按文件删除能力）──

#[test]
fn delete_file_removes_from_memory_and_disk() {
    // 双文件场景：update_file 会把写入目标记为 active_file，
    // 因此先写 main.cpp 再写 Main.java，active 切到后者后才能删前者
    let (mgr, storage) = test_manager_with_storage("delete-file-both");
    mgr.create("contest-6", "problem-F", "/ws").unwrap();
    mgr.update_file("main.cpp", "// old cpp").unwrap();
    mgr.update_file("Main.java", "// new java").unwrap();
    mgr.save().unwrap();

    mgr.delete_file("main.cpp").unwrap();

    let ws = mgr.current().unwrap();
    assert!(!ws.files.contains_key("main.cpp"), "内存中应已移除");
    assert!(ws.files.contains_key("Main.java"), "active 文件不受影响");

    let repo = FsWorkspaceRepository::new(Arc::clone(&storage));
    assert!(
        repo.read_file(&ws.id, &std::path::PathBuf::from("main.cpp"))
            .is_err(),
        "磁盘上应已删除"
    );
    assert_eq!(
        repo.read_file(&ws.id, &std::path::PathBuf::from("Main.java"))
            .unwrap(),
        "// new java"
    );
}

#[test]
fn delete_file_rejects_active_file() {
    // active_file 是选手当前代码，删除只针对过期文件
    let mgr = test_manager("delete-file-active");
    mgr.create("contest-6", "problem-F", "/ws").unwrap();
    mgr.update_file("main.cpp", "// current").unwrap();

    let result = mgr.delete_file("main.cpp");

    assert!(result.is_err(), "不得删除当前代码文件");
    let ws = mgr.current().unwrap();
    assert!(ws.files.contains_key("main.cpp"), "被拒后文件必须保留");
}

#[test]
fn delete_file_rejects_workspace_meta() {
    // 元数据被删会让工作区无法加载，必须显式拒绝
    let mgr = test_manager("delete-file-meta");
    mgr.create("contest-6", "problem-F", "/ws").unwrap();

    assert!(mgr.delete_file("workspace.json").is_err());
}

#[test]
fn guard_equivalent_normalizes_windows_name_variants() {
    // Windows 把大小写变体与尾随点/空格解析到同一文件，守卫比较必须归一
    assert_eq!(guard_equivalent("Workspace.json"), "workspace.json");
    assert_eq!(guard_equivalent("workspace.json."), "workspace.json");
    assert_eq!(guard_equivalent("workspace.json. . "), "workspace.json");
    assert_eq!(guard_equivalent("MAIN.JAVA"), "main.java");
    assert_eq!(guard_equivalent("Main.java "), "main.java");
    assert_eq!(guard_equivalent("main.cpp"), "main.cpp");
}

#[test]
fn delete_file_guards_reject_windows_name_variants() {
    // 精确比较会被变体名绕过（本机实测：Workspace.json / workspace.json.
    // 都命中 workspace.json，删除变体名会删掉原件）—— 守卫必须按等价类比较
    let mgr = test_manager("delete-file-variants");
    mgr.create("contest-6", "problem-F", "/ws").unwrap();
    mgr.update_file("Main.java", "// active").unwrap();

    assert!(
        mgr.delete_file("Workspace.json").is_err(),
        "大小写变体不得绕过元数据守卫"
    );
    assert!(
        mgr.delete_file("workspace.json.").is_err(),
        "尾随点变体不得绕过元数据守卫"
    );
    assert!(
        mgr.delete_file("workspace.json. ").is_err(),
        "尾随点+空格变体不得绕过元数据守卫"
    );
    assert!(
        mgr.delete_file("MAIN.JAVA").is_err(),
        "大小写变体不得绕过 active 守卫"
    );
    assert!(
        mgr.delete_file("Main.java.").is_err(),
        "尾随点变体不得绕过 active 守卫"
    );

    // 受保护文件原样保留
    let ws = mgr.current().unwrap();
    assert!(ws.files.contains_key("Main.java"));
}

#[test]
fn delete_file_rejects_ads_names_end_to_end() {
    // ADS 变体（workspace.json::$DATA = 文件本身的默认数据流）能绕过 manager
    // 的等价类守卫（"workspace.json::$data" ≠ "workspace.json"），由仓库层的
    // Win32 非法字符校验收口 —— 评审实测复现的完整链路，端到端锁定
    let (mgr, storage) = test_manager_with_storage("delete-file-ads");
    mgr.create("contest-6", "problem-F", "/ws").unwrap();
    mgr.update_file("Main.java", "// active").unwrap();
    mgr.save().unwrap();

    assert!(mgr.delete_file("workspace.json::$DATA").is_err());
    assert!(mgr.delete_file("Main.java::$DATA").is_err());

    // 受保护文件在内存与磁盘上均原样保留
    let ws = mgr.current().unwrap();
    assert!(ws.files.contains_key("Main.java"));
    let repo = FsWorkspaceRepository::new(Arc::clone(&storage));
    assert_eq!(
        repo.read_file(&ws.id, &std::path::PathBuf::from("Main.java"))
            .unwrap(),
        "// active"
    );
    assert!(mgr.load(&ws.id, "/ws").is_ok(), "元数据未被删除，工作区仍可加载");
}

#[test]
fn delete_file_rejects_without_current_workspace() {
    let mgr = test_manager("delete-file-no-current");
    assert!(mgr.delete_file("main.cpp").is_err());
}

#[test]
fn delete_file_missing_everywhere_is_noop() {
    // 幂等：内存与磁盘均无此文件时成功返回（清理路径可安全重试）
    let mgr = test_manager("delete-file-noop");
    mgr.create("contest-6", "problem-F", "/ws").unwrap();
    mgr.update_file("Main.java", "// keep").unwrap();

    mgr.delete_file("ghost.cpp").unwrap();

    let ws = mgr.current().unwrap();
    assert!(ws.files.contains_key("Main.java"));
    assert_eq!(mgr.get_file("Main.java").unwrap(), "// keep");
}

#[test]
fn delete_file_disk_only_file_still_removed_from_memory() {
    // 历史工作区：文件在磁盘上、但本次会话内存里没有（load 前的清理场景兜底）——
    // 磁盘删除后内存状态保持一致（无幽灵键）
    let (mgr, storage) = test_manager_with_storage("delete-file-disk-only");
    let ws = mgr.create("contest-6", "problem-F", "/ws").unwrap();
    mgr.update_file("main.cpp", "// stale").unwrap();
    // Main.java 最后写入 → active_file 指向它，main.cpp 才是可清理的过期文件
    mgr.update_file("Main.java", "// keep").unwrap();
    mgr.save().unwrap();
    // 模拟「内存里没有 main.cpp」：直接从 HashMap 移除（磁盘仍在）
    {
        let mut current = mgr.current.write().unwrap_or_else(|e| e.into_inner());
        current
            .as_mut()
            .unwrap()
            .files
            .remove("main.cpp");
    }

    mgr.delete_file("main.cpp").unwrap();

    let repo = FsWorkspaceRepository::new(Arc::clone(&storage));
    assert!(
        repo.read_file(&ws.id, &std::path::PathBuf::from("main.cpp"))
            .is_err(),
        "磁盘上的孤儿文件也应被删除"
    );
    assert_eq!(
        repo.read_file(&ws.id, &std::path::PathBuf::from("Main.java"))
            .unwrap(),
        "// keep"
    );
}

#[test]
fn load_recovers_workspace_from_disk() {
    let dir = TempDir::named("hinina-test-mgr-load-recover");
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let repo1 = Arc::new(FsWorkspaceRepository::new(Arc::clone(&storage)));
    let mgr1 = WorkspaceManager::new(repo1, Arc::new(CoreEventBus::new()));

    let ws = mgr1
        .create("contest-6", "problem-F", "/home/user/oj")
        .unwrap();
    let ws_id = ws.id.clone();
    mgr1.update_file("main.cpp", "// recovered file").unwrap();
    mgr1.save().unwrap(); // 内存是唯一权威副本：跨实例恢复前必须落盘

    // create a second manager sharing the same storage
    let repo2 = Arc::new(FsWorkspaceRepository::new(storage));
    let mgr2 = WorkspaceManager::new(repo2, Arc::new(CoreEventBus::new()));

    let loaded = mgr2.load(&ws_id, "/home/user/oj").unwrap();
    assert_eq!(loaded.contest_id, "contest-6");
    assert_eq!(loaded.problem_id, "problem-F");
    assert_eq!(loaded.files.get("main.cpp").unwrap(), "// recovered file");
}

#[test]
fn load_uses_metadata_not_id_parsing() {
    let dir = TempDir::named("hinina-test-mgr-metadata");
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let repo1 = Arc::new(FsWorkspaceRepository::new(Arc::clone(&storage)));
    let mgr1 = WorkspaceManager::new(repo1, Arc::new(CoreEventBus::new()));

    let ws = mgr1.create("contest-2024", "problem-G", "/ws").unwrap();
    let ws_id = ws.id.clone();
    mgr1.update_file("main.cpp", "// 2024 contest").unwrap();
    mgr1.save().unwrap(); // 内存是唯一权威副本：跨实例恢复前必须落盘

    let repo2 = Arc::new(FsWorkspaceRepository::new(storage));
    let mgr2 = WorkspaceManager::new(repo2, Arc::new(CoreEventBus::new()));

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
    let ws = mgr.create("contest-7", "problem-H", "/ws").unwrap();
    let ws_id = ws.id.clone();

    let repo = FsWorkspaceRepository::new(Arc::clone(&storage));
    assert!(repo.exists(&ws_id), "workspace should exist before destroy");

    mgr.destroy(&ws_id).unwrap();
    assert!(
        !repo.exists(&ws_id),
        "workspace should not exist after destroy"
    );
}

#[test]
fn switch_saves_current_and_loads_target() {
    let mgr = test_manager("switch");

    let ws1 = mgr.create("contest-8", "problem-I", "/ws").unwrap();
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

    let ws = mgr.create("contest-10", "problem-K", "/ws").unwrap();
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
    let mgr2 = WorkspaceManager::new(repo, Arc::new(CoreEventBus::new()));
    let loaded = mgr2.load(&ws.id, "/ws").expect("加载工作区失败");
    assert_eq!(loaded.language, "java", "语言必须跨实例持久化");
}

#[test]
fn save_persists_language_metadata() {
    let (mgr, storage) = test_manager_with_storage("save-language");
    let ws = mgr
        .create("contest-1", "problem-A", "/ws")
        .expect("创建工作区失败");
    mgr.update_file("Main.java", "class Main {}")
        .expect("写入文件失败");
    mgr.set_language("java").expect("设置语言失败");
    mgr.save().expect("保存失败");

    let repo = Arc::new(FsWorkspaceRepository::new(Arc::clone(&storage)));
    let mgr2 = WorkspaceManager::new(repo, Arc::new(CoreEventBus::new()));
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

use crate::core::event::core_event::CoreEvent;
use crate::core::repository::workspace_repo::WorkspaceRepository;

/// 写盘钩子：在真实写盘前执行（返回 Err 即模拟写失败）
type SaveHook = Arc<dyn Fn(&str, &str) -> AppResult<()> + Send + Sync>;

/// 可注入副作用的仓库桩：在真实写盘前/后执行钩子。
///
/// `on_save` 用于确定性复现两种时序（写盘期间编辑器又推了新内容、写盘失败）；
/// `on_saved` 用于等待「某一笔写确已落盘」（写盘后的完成信号，避免测试读到中间态）。
struct HookedRepo {
    inner: Arc<FsWorkspaceRepository>,
    on_save: SaveHook,
    on_saved: Option<SaveHook>,
}

impl WorkspaceRepository for HookedRepo {
    fn save_file(&self, workspace_id: &str, path: &Path, content: &str) -> AppResult<()> {
        (self.on_save)(workspace_id, content)?;
        let result = self.inner.save_file(workspace_id, path, content);
        if let Some(on_saved) = &self.on_saved {
            let _ = on_saved(workspace_id, content);
        }
        result
    }

    fn read_file(&self, workspace_id: &str, path: &Path) -> AppResult<String> {
        self.inner.read_file(workspace_id, path)
    }

    fn list_files(&self, workspace_id: &str) -> AppResult<Vec<std::path::PathBuf>> {
        self.inner.list_files(workspace_id)
    }

    fn delete_file(&self, workspace_id: &str, path: &Path) -> AppResult<()> {
        self.inner.delete_file(workspace_id, path)
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

/// 订阅「工作区已落盘」事件并返回计数句柄。
///
/// `WorkspaceSaved` 的 `automatic` 区分显式保存与 auto-save；这里只数 auto-save 的那些
/// （与旧 `AutoSaveTriggered` 的语义一致）。
fn count_auto_save_events(bus: &Arc<CoreEventBus>) -> Arc<AtomicUsize> {
    let count = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&count);
    let mut rx = bus.subscribe();
    std::thread::spawn(move || loop {
        match rx.blocking_recv() {
            Ok(CoreEvent::WorkspaceSaved {
                automatic: true, ..
            }) => {
                counter.fetch_add(1, AtomicOrdering::SeqCst);
            }
            Ok(_) => {}
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
        }
    });
    count
}

/// auto-save 清脏判据的确定性锁定。
///
/// 集成测试只能观测到「最终磁盘/脏标记」，判据被削弱（如删掉修订号比较）时它
/// 在多数时序下仍会通过 —— 这里逐条钉死判据本身。
#[test]
fn can_mark_clean_requires_same_workspace_and_unchanged_revision() {
    assert!(
        can_mark_clean("ws-1", "ws-1", 7, 7),
        "同一工作区且修订号未变：可以清脏"
    );
    assert!(
        !can_mark_clean("ws-1", "ws-1", 7, 8),
        "快照之后有新改动：不得清脏（否则新内容永不落盘）"
    );
    assert!(
        !can_mark_clean("ws-1", "ws-2", 7, 7),
        "已切到别的工作区：不得清新工作区的脏标记"
    );
    assert!(!can_mark_clean("ws-1", "ws-2", 7, 8));
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
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auto_save_write_is_serialized_with_explicit_save() {
    // 竞态复现（确定性）：auto-save 取快照后写盘期间，显式 save() 写入更新的内容。
    // 若 auto-save 的写盘不持锁，旧快照的写会**落在新内容之后** → 磁盘回退到 v1；
    // 而工作区在 save() 完成时已标 clean（rev 检查只能阻止 auto-save 误标 clean，
    // 无法撤销已发生的覆盖）→ 内存 v2 且永不重写 = 最后一次编辑静默永久丢失。
    let dir = TempDir::named("hinina-test-mgr-autosave-serialize");
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let real_repo = Arc::new(FsWorkspaceRepository::new(Arc::clone(&storage)));
    let bus = Arc::new(CoreEventBus::new());

    // 钩子：auto-save 写 "v1" 时通知测试并阻塞（模拟被抢占 / 慢盘 / 杀毒扫描）；
    // 写盘完成后再发一次信号，供测试确定性等待「旧写已落盘」
    let (tx_started, rx_started) = std::sync::mpsc::channel::<()>();
    let (tx_release, rx_release) = std::sync::mpsc::channel::<()>();
    let (tx_written, rx_written) = std::sync::mpsc::channel::<()>();
    let tx_started = Arc::new(std::sync::Mutex::new(tx_started));
    let rx_release = Arc::new(std::sync::Mutex::new(rx_release));
    let tx_written = Arc::new(std::sync::Mutex::new(tx_written));
    let on_save: SaveHook = Arc::new(move |_ws_id, content| {
        if content == "v1" {
            let _ = tx_started.lock().unwrap().send(());
            let _ = rx_release.lock().unwrap().recv();
        }
        Ok(())
    });
    let on_saved: SaveHook = Arc::new(move |_ws_id, content| {
        if content == "v1" {
            let _ = tx_written.lock().unwrap().send(());
        }
        Ok(())
    });

    let repo: Arc<dyn WorkspaceRepository> = Arc::new(HookedRepo {
        inner: Arc::clone(&real_repo),
        on_save,
        on_saved: Some(on_saved),
    });
    let mgr = Arc::new(WorkspaceManager::new(repo, Arc::clone(&bus)));

    mgr.create("c", "p", "/ws").unwrap();
    mgr.update_file("main.cpp", "v1").unwrap();
    mgr.start_auto_save(1);

    // 等到 auto-save 确实进入写盘阶段（此刻它应当持有读锁）
    rx_started
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("auto-save 未在 5 秒内进入写盘");

    // 另一线程做「推入 v2 + 显式落盘」——即前端失焦/切题触发的 saveWorkspace
    let mgr_for_thread = Arc::clone(&mgr);
    let saver = std::thread::spawn(move || {
        mgr_for_thread.update_file("main.cpp", "v2").unwrap();
        mgr_for_thread.save().unwrap();
    });

    // 给显式保存足够时间完成（若写盘不受锁保护，它会在 auto-save 的写盘之前完成）
    std::thread::sleep(std::time::Duration::from_millis(300));

    // 放行被阻塞的 auto-save 写盘，并确定性等待它落盘完成
    let _ = tx_release.send(());
    rx_written
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("auto-save 的旧快照写盘未在 5 秒内完成");
    saver.join().expect("显式保存线程 panic");

    let ws = mgr.current().unwrap();
    let on_disk = real_repo
        .read_file(&ws.id, &std::path::PathBuf::from("main.cpp"))
        .unwrap();

    assert_eq!(
        on_disk, "v2",
        "最新内容必须是最后一次落盘（旧快照的写不得落在新内容之后）"
    );
    assert!(!ws.is_dirty, "v2 已落盘，工作区应为干净");
    assert_eq!(ws.files.get("main.cpp").map(String::as_str), Some("v2"));

    mgr.stop_auto_save();
}

#[tokio::test]
async fn auto_save_eventually_persists_newest_content_when_edit_arrives_mid_tick() {
    // tick 期间到来的新改动不得被永久吞掉：无论竞态哪一方先拿到锁，
    // 最终磁盘必须是新内容且工作区干净（不允许「clean 但磁盘落后」的终态）。
    let dir = TempDir::named("hinina-test-mgr-autosave-midtick");
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let real_repo = Arc::new(FsWorkspaceRepository::new(Arc::clone(&storage)));
    let bus = Arc::new(CoreEventBus::new());
    let events = count_auto_save_events(&bus);

    let slot: Arc<std::sync::Mutex<Option<Arc<WorkspaceManager>>>> =
        Arc::new(std::sync::Mutex::new(None));
    let hook_slot = Arc::clone(&slot);
    let on_save: SaveHook = Arc::new(move |_ws_id, content| {
        if content == "v1" {
            // 写盘期间推入新改动（另一线程，避免持锁自重入）
            if let Some(mgr) = hook_slot.lock().unwrap().clone() {
                std::thread::spawn(move || {
                    mgr.update_file("main.cpp", "v2").unwrap();
                });
            }
        }
        Ok(())
    });

    let repo: Arc<dyn WorkspaceRepository> = Arc::new(HookedRepo {
        inner: Arc::clone(&real_repo),
        on_save,
        on_saved: None,
    });
    let mgr = Arc::new(WorkspaceManager::new(repo, Arc::clone(&bus)));
    *slot.lock().unwrap() = Some(Arc::clone(&mgr));

    mgr.create("c", "p", "/ws").unwrap();
    mgr.update_file("main.cpp", "v1").unwrap();

    mgr.start_auto_save(1);
    // 两个 tick 足够让「新改动」被重新快照并落盘
    tokio::time::sleep(std::time::Duration::from_millis(2_500)).await;
    mgr.stop_auto_save();

    let ws = mgr.current().unwrap();
    assert_eq!(
        real_repo
            .read_file(&ws.id, &std::path::PathBuf::from("main.cpp"))
            .unwrap(),
        "v2",
        "tick 期间的新改动最终必须落盘"
    );
    assert!(!ws.is_dirty, "落盘后应标记干净");
    // 事件数取决于锁竞争结果，合法取值 1 或 2（实测 1,2,2,2,1,2）：
    // - 新改动先拿到写锁 → 本 tick 修订号已变 → 不发布；下 tick 落盘并发布 → 1
    // - 本 tick 先拿到写锁 → 本 tick 发布；新改动随后标脏 → 下 tick 再发布 → 2
    // 两者都是正确行为，故此处**不收紧为 == 1**（会 flaky）。判据本身由
    // `can_mark_clean_requires_same_workspace_and_unchanged_revision` 确定性锁定。
    assert!(
        events.load(AtomicOrdering::SeqCst) >= 1,
        "至少发布一次「已落盘」事件"
    );
}

#[tokio::test]
async fn auto_save_keeps_dirty_and_silent_when_write_fails() {
    let dir = TempDir::named("hinina-test-mgr-autosave-fail");
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let real_repo = Arc::new(FsWorkspaceRepository::new(Arc::clone(&storage)));
    let bus = Arc::new(CoreEventBus::new());
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
        on_saved: None,
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
}

/// auto-save 的启停与间隔是可观测状态，且支持「停掉再启动」（P48）。
#[tokio::test]
async fn auto_save_tracks_interval_and_can_be_restarted() {
    let mgr = test_manager("auto-save-restart");
    assert_eq!(mgr.auto_save_interval_secs(), None, "初始未运行");

    mgr.start_auto_save(1);
    assert_eq!(mgr.auto_save_interval_secs(), Some(1));

    // 配置把间隔改了：以新间隔重启，而不是被一次性标记挡住
    mgr.start_auto_save(2);
    assert_eq!(mgr.auto_save_interval_secs(), Some(2));

    mgr.stop_auto_save();
    assert_eq!(
        mgr.auto_save_interval_secs(),
        None,
        "停止后不得残留运行状态"
    );

    // 「关掉再打开」必须能重新启动 —— 旧的一次性 static 标记正是在这里失败
    mgr.start_auto_save(3);
    assert_eq!(mgr.auto_save_interval_secs(), Some(3));
    mgr.stop_auto_save();
}

/// `activeFile` 是当前代码文件的权威源：`update_file` 写到哪个文件就记哪个（P62）。
#[test]
fn update_file_records_active_file() {
    let mgr = test_manager("active-file-update");
    mgr.create("contest-af", "problem-af", "/ws").unwrap();
    assert_eq!(
        mgr.current().unwrap().active_file,
        None,
        "新工作区尚无代码文件"
    );

    mgr.update_file("main.cpp", "v1").unwrap();
    assert_eq!(
        mgr.current().unwrap().active_file.as_deref(),
        Some("main.cpp")
    );

    // 语言切换后前端会把代码推到新文件名 —— 权威源必须跟着走，
    // 否则加载路径又会退回「按语言派生 + 后缀探测」去猜
    mgr.update_file("Main.java", "v2").unwrap();
    assert_eq!(
        mgr.current().unwrap().active_file.as_deref(),
        Some("Main.java")
    );
}

/// `activeFile` 随元数据落盘，并在重新加载后恢复（重启后不必再猜）。
#[test]
fn active_file_survives_reload() {
    let dir = TempDir::named("hinina-test-mgr-active-file-persist");
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let mgr1 = WorkspaceManager::new(
        Arc::new(FsWorkspaceRepository::new(Arc::clone(&storage))),
        Arc::new(CoreEventBus::new()),
    );

    let ws_id = mgr1.create("contest-afp", "problem-afp", "/ws").unwrap().id;
    // 模拟「语言切到 Java 后写入新文件」：两个代码文件并存
    mgr1.update_file("main.cpp", "// 旧 C++ 代码").unwrap();
    mgr1.update_file("Main.java", "class Main {}").unwrap();
    mgr1.save().unwrap();

    let mgr2 = WorkspaceManager::new(
        Arc::new(FsWorkspaceRepository::new(storage)),
        Arc::new(CoreEventBus::new()),
    );
    let loaded = mgr2.load(&ws_id, "/ws").unwrap();

    assert_eq!(
        loaded.active_file.as_deref(),
        Some("Main.java"),
        "重启后必须仍指向语言切换后的那个文件，而不是按后缀探测碰运气"
    );
    assert!(
        loaded.files.contains_key("main.cpp"),
        "旧文件仍在（P62 未闭合部分）"
    );
}

/// 历史 workspace.json 没有 `activeFile` 字段：反序列化为 `None`，不得报错。
#[test]
fn legacy_meta_without_active_file_loads_as_none() {
    let dir = TempDir::named("hinina-test-mgr-active-file-legacy");
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let repo = Arc::new(FsWorkspaceRepository::new(Arc::clone(&storage)));
    let mgr = WorkspaceManager::new(
        Arc::clone(&repo) as Arc<dyn WorkspaceRepository>,
        Arc::new(CoreEventBus::new()),
    );

    let ws_id = mgr
        .create("contest-legacy", "problem-legacy", "/ws")
        .unwrap()
        .id;
    // 用「旧格式」元数据覆盖：无 active_file 字段（WorkspaceMeta 的线上格式是 snake_case）
    let legacy = r#"{"contest_id":"contest-legacy","problem_id":"problem-legacy","root_path":"/ws","language":"C++","created_at":1,"updated_at":1}"#;
    repo.save_file(&ws_id, &std::path::PathBuf::from("workspace.json"), legacy)
        .unwrap();

    let loaded = mgr.load(&ws_id, "/ws").unwrap();
    assert_eq!(loaded.active_file, None, "旧元数据应降级为 None 而不是报错");
    assert_eq!(loaded.language, "C++");
}
/// 并发 `start_auto_save` 不得泄漏无法停止的孤儿循环。
///
/// 「停旧的 → 起新的 → 登记句柄」分三次取锁时，后登记者会覆盖先登记的句柄 ——
/// 先起的循环从此无人可停，`stop_auto_save` 之后仍按自己的节拍写盘（表现为
/// 「关了自动保存却还在保存」）。
///
/// 判据：全部并发启动完成后停一次，再把工作区**弄脏**静候两个节拍 —— 只有仍在跑的
/// 孤儿循环会把它写回干净。注意这是概率性复现（窗口很窄），断言本身才是长期防线。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_start_auto_save_does_not_leak_orphan_tasks() {
    let dir = TempDir::named("hinina-test-mgr-autosave-race");
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let repo = Arc::new(FsWorkspaceRepository::new(Arc::clone(&storage)));
    let mgr = Arc::new(WorkspaceManager::new(repo, Arc::new(CoreEventBus::new())));
    mgr.create("c", "p", "/ws").unwrap();

    // std 线程没有 tokio 上下文，显式 enter 后再调（start_auto_save 内部要 tokio::spawn）
    let rt = tokio::runtime::Handle::current();
    let mut threads = Vec::new();
    for _ in 0..8 {
        let mgr = Arc::clone(&mgr);
        let rt = rt.clone();
        threads.push(std::thread::spawn(move || {
            let _guard = rt.enter();
            mgr.start_auto_save(1);
        }));
    }
    for t in threads {
        t.join().unwrap();
    }

    mgr.stop_auto_save();
    assert_eq!(
        mgr.auto_save_interval_secs(),
        None,
        "停止后不得残留运行状态"
    );

    mgr.update_file("main.cpp", "after-stop").unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(2_500)).await;
    assert!(
        mgr.current().unwrap().is_dirty,
        "stop_auto_save 之后不得再有任何循环在跑（孤儿任务会把脏标记清掉）"
    );
}

// ── `WorkspaceSaved` 事件契约（「最新内容确已在磁盘上」）──

/// 显式落盘发布 `WorkspaceSaved { revision, automatic: false }`。
///
/// `revision` 让前端能丢弃过期事件（切题前保存旧工作区的事件可能在新工作区
/// 已加载之后才送达）；`automatic` 区分「手动保存」与「后台自动备份」。
#[test]
fn save_publishes_workspace_saved_with_revision() {
    let (mgr, _storage, bus) = test_manager_with_bus("saved-event");
    let ws = mgr.create("c", "p", "/ws").expect("创建工作区失败");
    let mut rx = bus.subscribe();

    mgr.update_file("main.cpp", "v1").expect("写入失败");
    mgr.save().expect("保存失败");

    assert_eq!(
        rx.try_recv().expect("落盘成功应发布 WorkspaceSaved"),
        CoreEvent::WorkspaceSaved {
            workspace_id: ws.id.clone(),
            revision: 1,
            automatic: false,
        }
    );
}

/// 未脏时保存是 no-op：**不得**发布「已落盘」事件（事件等价于磁盘真值声明）。
#[test]
fn save_without_changes_publishes_nothing() {
    let (mgr, _storage, bus) = test_manager_with_bus("saved-event-clean");
    mgr.create("c", "p", "/ws").expect("创建工作区失败");
    mgr.update_file("main.cpp", "v1").expect("写入失败");
    mgr.save().expect("首次保存失败");

    let mut rx = bus.subscribe();
    mgr.save().expect("二次保存（未脏）应成功");

    assert!(
        rx.try_recv().is_err(),
        "未脏的保存是 no-op，不应发布「已落盘」事件"
    );
}

/// 落盘失败时**不发布**事件：事件表示「最新内容确已在磁盘上」，
/// 失败却发事件会让前端显示「已自动备份」而磁盘其实落后。
#[test]
fn save_failure_does_not_publish_workspace_saved() {
    let dir = TempDir::named("hinina-test-mgr-saved-event-fail");
    let storage = Arc::new(Storage::new(dir.to_path_buf()));
    let real_repo = Arc::new(FsWorkspaceRepository::new(Arc::clone(&storage)));
    let bus = Arc::new(CoreEventBus::new());

    // 钩子：内容为 "boom" 时写盘失败（确定性，不依赖权限或只读卷）
    let on_save: SaveHook = Arc::new(|_ws_id, content| {
        if content == "boom" {
            return Err(AppError::Io("模拟磁盘写失败".into()));
        }
        Ok(())
    });
    let repo = Arc::new(HookedRepo {
        inner: real_repo,
        on_save,
        on_saved: None,
    });
    let mgr = WorkspaceManager::new(repo, Arc::clone(&bus));
    mgr.create("c", "p", "/ws").expect("创建工作区失败");
    let mut rx = bus.subscribe();

    mgr.update_file("main.cpp", "boom").expect("写入失败");
    assert!(mgr.save().is_err(), "写盘失败应如实报错");
    assert!(
        rx.try_recv().is_err(),
        "落盘失败不得发布 WorkspaceSaved（前端会误显示「已自动备份」）"
    );
}

/// 切换工作区前**显式保存旧工作区**，且事件带的是**旧**工作区 id。
///
/// 前端按 workspace_id 过滤过期事件：不带 id 就无从分辨「刚保存的是旧工作区」，
/// 会误清新工作区的脏标记。
#[test]
fn switch_workspace_saves_old_workspace_before_replacing() {
    let (mgr, _storage, bus) = test_manager_with_bus("switch-saves-old");
    // create 会把 current 切到新工作区，因此两个都建好后再切回 old
    let target = mgr.create("c", "p2", "/ws").expect("创建目标工作区失败");
    let old = mgr.create("c", "p1", "/ws").expect("创建旧工作区失败");
    mgr.load(&old.id, "/ws").expect("加载旧工作区失败");
    mgr.update_file("main.cpp", "unsaved-old-content")
        .expect("写入失败");
    let mut rx = bus.subscribe();

    mgr.switch(&target.id, "/ws").expect("切换失败");

    assert_eq!(
        rx.try_recv().expect("切换前应显式保存旧工作区并发布事件"),
        CoreEvent::WorkspaceSaved {
            workspace_id: old.id.clone(),
            revision: 1,
            automatic: false,
        },
        "事件必须指向被保存的**旧**工作区，且 revision 与保存时的内容一致"
    );
    assert_eq!(
        mgr.current().map(|ws| ws.id),
        Some(target.id),
        "切换后当前工作区应是目标工作区"
    );
}
