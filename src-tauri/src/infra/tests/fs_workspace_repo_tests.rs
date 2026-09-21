use super::*;
use crate::test_support::{Guarded, TempDir};
use std::path::PathBuf;

/// 测试仓库 + 临时目录守卫（`Drop` 时删除目录，避免临时文件堆积）。
fn repo(name: &str) -> Guarded<FsWorkspaceRepository> {
    let dir = TempDir::named(&format!("hinina-test-ws-{}", name));
    let repo = FsWorkspaceRepository::new(Arc::new(Storage::new(dir.to_path_buf())));
    Guarded::new(repo, dir)
}

#[test]
fn save_and_read_file() {
    let r = repo("save-read");
    let ws = "test-ws-1";
    let path = Path::new("main.cpp");
    let content = "#include <iostream>\nint main() { return 0; }";

    r.save_file(ws, path, content).unwrap();
    assert!(r.exists(ws));

    let read = r.read_file(ws, path).unwrap();
    assert_eq!(read, content);
}

#[test]
fn read_nonexistent_file_returns_error() {
    let r = repo("no-file");
    let result = r.read_file("no-such-ws", Path::new("x.cpp"));
    assert!(result.is_err());
}

#[test]
fn list_files_after_save() {
    let r = repo("list-files");
    let ws = "test-ws-2";
    r.save_file(ws, Path::new("a.cpp"), "// a").unwrap();
    r.save_file(ws, Path::new("sub/b.py"), "# b").unwrap();

    let mut files = r.list_files(ws).unwrap();
    files.sort();
    assert_eq!(files.len(), 2);
    assert!(files.contains(&PathBuf::from("a.cpp")));
    assert!(files.contains(&PathBuf::from("sub/b.py")));
}

#[test]
fn list_files_nonexistent_workspace_returns_empty() {
    let r = repo("empty-list");
    let files = r.list_files("no-such-ws").unwrap();
    assert!(files.is_empty());
}

#[test]
fn exists_detects_workspace() {
    let r = repo("exists");
    assert!(!r.exists("ws-x"));
    r.save_file("ws-x", Path::new("f.txt"), "hello").unwrap();
    assert!(r.exists("ws-x"));
}

#[test]
fn delete_workspace_removes_all() {
    let r = repo("delete");
    let ws = "ws-to-delete";
    r.save_file(ws, Path::new("f.txt"), "data").unwrap();
    assert!(r.exists(ws));

    r.delete_workspace(ws).unwrap();
    assert!(!r.exists(ws));
    assert!(r.list_files(ws).unwrap().is_empty());
}

#[test]
fn rejects_path_traversal() {
    let r = repo("traversal");
    let ws = "ws-safe";
    // 确保 workspace 存在
    r.save_file(ws, Path::new("legit.txt"), "ok").unwrap();

    assert!(r.save_file(ws, Path::new("../escape.txt"), "bad").is_err());
    assert!(r.read_file(ws, Path::new("../../etc/passwd")).is_err());
    assert!(r.save_file(ws, Path::new("a/../b"), "bad").is_err());
}

// ── validate_workspace_id 边界测试 ──

#[test]
fn validate_workspace_id_rejects_slash() {
    // workspace_id 含 / 应被拒绝（路径穿越风险）
    assert!(FsWorkspaceRepository::validate_workspace_id("ws/../etc").is_err());
    assert!(FsWorkspaceRepository::validate_workspace_id("a/b").is_err());
}

#[test]
fn validate_workspace_id_rejects_dotdot() {
    // workspace_id 含 .. 应被拒绝
    assert!(FsWorkspaceRepository::validate_workspace_id("ws-..-foo").is_err());
    assert!(FsWorkspaceRepository::validate_workspace_id("..").is_err());
}

#[test]
fn validate_workspace_id_rejects_empty() {
    assert!(FsWorkspaceRepository::validate_workspace_id("").is_err());
}

#[test]
fn validate_workspace_id_rejects_non_ascii() {
    // 中文或空格应被拒绝
    assert!(FsWorkspaceRepository::validate_workspace_id("你好").is_err());
    assert!(FsWorkspaceRepository::validate_workspace_id("ws bad").is_err());
}

#[test]
fn validate_workspace_id_accepts_valid() {
    // 合法 ID 应通过
    assert!(FsWorkspaceRepository::validate_workspace_id("ws-contest1-problemA-1234567890-abcd").is_ok());
    assert!(FsWorkspaceRepository::validate_workspace_id("my_workspace").is_ok());
    assert!(FsWorkspaceRepository::validate_workspace_id("abc123").is_ok());
}
