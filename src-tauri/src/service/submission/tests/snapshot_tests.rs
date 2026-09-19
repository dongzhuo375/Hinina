// snapshot.rs 单元测试：扩展名推导、路径构造与安全、落盘/读取与回退扫描

use super::*;

use std::path::PathBuf;

/// 临时目录下建一个独立存储根（每个测试一个，避免相互干扰）
fn temp_storage(tag: &str) -> (Storage, PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "hinina-snapshot-test-{}-{}",
        tag,
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    (Storage::new(dir.clone()), dir)
}

// ── source_extension ──

#[test]
fn source_extension_matches_frontend_source_file_name_of() {
    // 与前端 utils/language.sourceFileNameOf 的后缀逐一对齐
    assert_eq!(source_extension("C"), "c");
    assert_eq!(source_extension("C++"), "cpp");
    assert_eq!(source_extension("Java"), "java");
    assert_eq!(source_extension("Kotlin"), "kt");
    assert_eq!(source_extension("Python"), "py");
    assert_eq!(source_extension("JavaScript"), "js");
    assert_eq!(source_extension("TypeScript"), "ts");
    assert_eq!(source_extension("Go"), "go");
    assert_eq!(source_extension("Rust"), "rs");
    assert_eq!(source_extension("C#"), "cs");
    assert_eq!(source_extension("PHP"), "php");
    assert_eq!(source_extension("Ruby"), "rb");
    assert_eq!(source_extension("SQL"), "sql");
}

#[test]
fn source_extension_handles_deployment_variants() {
    // 服务端允许列表里是部署变体，按前缀归一
    assert_eq!(source_extension("C++17 (GCC 13.2)"), "cpp");
    assert_eq!(source_extension("C++ 20 With O2"), "cpp");
    assert_eq!(source_extension("Python 3.10"), "py");
    assert_eq!(source_extension("PyPy3"), "py");
    assert_eq!(source_extension("Golang"), "go");
    assert_eq!(source_extension("C With O2"), "c");
}

#[test]
fn source_extension_does_not_swallow_sibling_families() {
    // "c#" 不能被 "c" 吞掉、"javascript" 不能被 "java" 吞掉（前缀判定的经典坑）
    assert_eq!(source_extension("C#"), "cs");
    assert_eq!(source_extension("JavaScript Node"), "js");
    assert_eq!(source_extension("JavaScript V8"), "js");
    assert_eq!(source_extension("Java"), "java");
}

#[test]
fn source_extension_unknown_language_falls_back_to_txt() {
    // 绝不猜 .cpp：判题端按后缀判语言，猜错等于让人误以为交的是另一种语言
    assert_eq!(source_extension("Brainfuck"), "txt");
    assert_eq!(source_extension(""), "txt");
    assert_eq!(source_extension("   "), "txt");
}

// ── snapshot_path ──

#[test]
fn snapshot_path_is_scoped_by_oj_and_submit_id() {
    let path = snapshot_path("HOJ", "1166", "C++").expect("路径构造失败");
    assert_eq!(path, "submissions/HOJ/1166.cpp");
}

#[test]
fn snapshot_path_rejects_path_traversal() {
    // 越权路径必须拒绝：submit_id / oj_id 来自服务端响应与会话，不得写出存储根之外
    assert!(snapshot_path("HOJ", "../evil", "C++").is_err());
    assert!(snapshot_path("HOJ", "a/b", "C++").is_err());
    assert!(snapshot_path("HOJ", "a\\b", "C++").is_err());
    assert!(snapshot_path("../evil", "1166", "C++").is_err());
    assert!(snapshot_path("", "1166", "C++").is_err());
}

// ── 落盘与读取 ──

#[test]
fn write_then_read_roundtrip() {
    let (storage, dir) = temp_storage("roundtrip");
    let code = "int main() { return 0; }";

    write_snapshot(&storage, "HOJ", "1166", "C++", code);

    let read = read_snapshot(&storage, "HOJ", "1166", "C++");
    assert_eq!(read.as_deref(), Some(code));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_falls_back_to_scan_when_language_changed() {
    // 提交时是 "C++"，查询时详情返回别的写法（服务端归一 / 选手改语言）：
    // 精确路径落空，但快照就在那儿，必须能扫出来
    let (storage, dir) = temp_storage("fallback");
    let code = "#include <bits/stdc++.h>";

    write_snapshot(&storage, "HOJ", "1166", "C++", code);

    let read = read_snapshot(&storage, "HOJ", "1166", "C++ 17");
    assert_eq!(read.as_deref(), Some(code));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_returns_none_when_absent() {
    let (storage, dir) = temp_storage("absent");
    assert_eq!(read_snapshot(&storage, "HOJ", "999999", "C++"), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn snapshots_of_different_ojs_do_not_collide() {
    // submit_id 是各 OJ 自增的资源号，跨 OJ 必然重号
    let (storage, dir) = temp_storage("oj-dimension");

    write_snapshot(&storage, "HOJ", "1", "C++", "hoj code");
    write_snapshot(&storage, "Hydro", "1", "C++", "hydro code");

    assert_eq!(
        read_snapshot(&storage, "HOJ", "1", "C++").as_deref(),
        Some("hoj code")
    );
    assert_eq!(
        read_snapshot(&storage, "Hydro", "1", "C++").as_deref(),
        Some("hydro code")
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_snapshot_failure_does_not_panic() {
    // best-effort 语义：非法 submit_id 只记录告警，不 panic、不阻断提交
    let (storage, dir) = temp_storage("best-effort");
    write_snapshot(&storage, "HOJ", "../evil", "C++", "code");
    // 未写出任何文件
    assert!(!storage.exists("submissions"));
    let _ = std::fs::remove_dir_all(&dir);
}
