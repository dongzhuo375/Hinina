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
fn save_persists_files_to_disk() {
    let (mgr, storage) = test_manager_with_storage("save-persist");
    mgr.create("contest-2", "problem-B", "/ws").unwrap();
    mgr.update_file("main.cpp", "int main() { return 0; }")
        .unwrap();

    // verify the file is already on disk (update_file persists immediately)
    let repo = FsWorkspaceRepository::new(Arc::clone(&storage));
    let ws = mgr.current().unwrap();
    let content = repo
        .read_file(&ws.id, &std::path::PathBuf::from("main.cpp"))
        .unwrap();
    assert_eq!(content, "int main() { return 0; }");

    mgr.save().unwrap();

    // after save, file should still be on disk
    let content = repo
        .read_file(&ws.id, &std::path::PathBuf::from("main.cpp"))
        .unwrap();
    assert_eq!(content, "int main() { return 0; }");
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
