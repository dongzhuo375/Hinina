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
fn delete_file_removes_disk_file() {
    let r = repo("delete-file");
    let ws = "ws-del";
    r.save_file(ws, Path::new("main.cpp"), "// old").unwrap();
    r.save_file(ws, Path::new("Main.java"), "// keep").unwrap();

    r.delete_file(ws, Path::new("main.cpp")).unwrap();

    assert!(r.read_file(ws, Path::new("main.cpp")).is_err());
    assert_eq!(r.read_file(ws, Path::new("Main.java")).unwrap(), "// keep");
}

#[test]
fn delete_file_missing_is_noop() {
    // 幂等：清理场景下重复调用不应报错
    let r = repo("delete-file-missing");
    let ws = "ws-del-miss";
    r.save_file(ws, Path::new("Main.java"), "// keep").unwrap();

    r.delete_file(ws, Path::new("ghost.cpp")).unwrap();

    assert_eq!(r.read_file(ws, Path::new("Main.java")).unwrap(), "// keep");
}

#[test]
fn delete_file_rejects_path_traversal() {
    let r = repo("delete-file-traversal");
    let ws = "ws-del-safe";
    r.save_file(ws, Path::new("legit.txt"), "ok").unwrap();

    assert!(r.delete_file(ws, Path::new("../escape.txt")).is_err());
    assert!(r.delete_file(ws, Path::new("a/../b")).is_err());
    // 合法文件未被波及
    assert_eq!(r.read_file(ws, Path::new("legit.txt")).unwrap(), "ok");
}

#[test]
fn rejects_non_normal_path_components() {
    // delete_file 是破坏性原语：组件白名单必须强于「只拒 ..」——
    // 绝对路径（根目录组件）、前导 `.`、空路径全部拒绝
    let r = repo("non-normal");
    let ws = "ws-safe2";
    r.save_file(ws, Path::new("legit.txt"), "ok").unwrap();

    assert!(r.save_file(ws, Path::new("/etc/passwd"), "bad").is_err());
    assert!(r.read_file(ws, Path::new("/etc/passwd")).is_err());
    assert!(r.delete_file(ws, Path::new("/etc/passwd")).is_err());
    assert!(r.delete_file(ws, Path::new("./legit.txt")).is_err());
    assert!(r.delete_file(ws, Path::new(".")).is_err());
    assert!(r.delete_file(ws, Path::new("")).is_err());
    // 合法文件不受影响
    assert_eq!(r.read_file(ws, Path::new("legit.txt")).unwrap(), "ok");
}

#[cfg(windows)]
#[test]
fn rejects_drive_letter_paths() {
    // Windows 盘符前缀（Prefix + RootDir 组件）必须拒绝
    let r = repo("drive-letter");
    let ws = "ws-safe3";
    r.save_file(ws, Path::new("legit.txt"), "ok").unwrap();

    assert!(r.delete_file(ws, Path::new("C:/Windows/system32/config")).is_err());
    assert!(r.save_file(ws, Path::new("C:/evil.txt"), "bad").is_err());
    assert_eq!(r.read_file(ws, Path::new("legit.txt")).unwrap(), "ok");
}

#[test]
fn rejects_ads_device_and_trailing_variant_paths() {
    // NTFS ADS：workspace.json::$DATA 是文件本身的默认数据流，fs::remove_file
    // 会删掉真正的 workspace.json —— manager 守卫的等价类比较拦不住
    // （"workspace.json::$data" ≠ "workspace.json"），必须在仓库层收口。
    // 读写删三原语同时生效，顺带堵住 save_file("workspace.json:evil") 造 ADS。
    let r = repo("ads-device");
    let ws = "ws-safe4";
    r.save_file(ws, Path::new("legit.txt"), "ok").unwrap();

    // ADS 删除绕过（评审实测复现的链路）
    assert!(r.delete_file(ws, Path::new("workspace.json::$DATA")).is_err());
    assert!(r.delete_file(ws, Path::new("Main.java::$DATA")).is_err());
    // ADS 写路径
    assert!(r.save_file(ws, Path::new("workspace.json:evil"), "bad").is_err());
    // 其余 Win32 非法字符
    assert!(r.save_file(ws, Path::new("a<b.txt"), "bad").is_err());
    assert!(r.save_file(ws, Path::new("a>b.txt"), "bad").is_err());
    assert!(r.save_file(ws, Path::new("a\"b.txt"), "bad").is_err());
    assert!(r.save_file(ws, Path::new("a|b.txt"), "bad").is_err());
    assert!(r.save_file(ws, Path::new("a?b.txt"), "bad").is_err());
    assert!(r.save_file(ws, Path::new("a*b.txt"), "bad").is_err());
    // 保留设备名（含带扩展名变体：Win32 按第一个点前的词干解析）
    assert!(r.delete_file(ws, Path::new("CON")).is_err());
    assert!(r.delete_file(ws, Path::new("NUL.txt")).is_err());
    assert!(r.save_file(ws, Path::new("COM1"), "bad").is_err());
    assert!(r.save_file(ws, Path::new("aux.js"), "bad").is_err());
    // 尾随点/空格（Windows 等价类，堵住 save_file("workspace.json.") 写路径）
    assert!(r.save_file(ws, Path::new("workspace.json."), "bad").is_err());
    assert!(r.save_file(ws, Path::new("main.cpp "), "bad").is_err());
    assert!(r.delete_file(ws, Path::new("legit.txt.")).is_err());
    // 合法文件不受影响
    assert_eq!(r.read_file(ws, Path::new("legit.txt")).unwrap(), "ok");
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
