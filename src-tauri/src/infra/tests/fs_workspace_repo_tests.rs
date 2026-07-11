use super::*;
use std::path::PathBuf;

fn repo(name: &str) -> FsWorkspaceRepository {
    let dir = std::env::temp_dir().join(format!("hinina-test-ws-{}", name));
    let _ = std::fs::remove_dir_all(&dir);
    FsWorkspaceRepository::new(Arc::new(Storage::new(dir)))
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
