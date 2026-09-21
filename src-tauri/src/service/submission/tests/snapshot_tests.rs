// snapshot.rs 单元测试：扩展名推导、路径构造与安全、落盘/读取与回退扫描

use super::*;

use crate::test_support::TempDir;

/// 临时目录下建一个独立存储根（每个测试一个，避免相互干扰）。
///
/// 返回守卫：目录随 `Drop` 回收，不再依赖每个用例末尾手工 `remove_dir_all`。
fn temp_storage(tag: &str) -> (Storage, TempDir) {
    let dir = TempDir::unique(&format!("hinina-snapshot-test-{tag}"));
    (Storage::new(dir.to_path_buf()), dir)
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
    let (storage, _dir) = temp_storage("roundtrip");
    let code = "int main() { return 0; }";

    write_snapshot(&storage, "HOJ", "1166", "C++", code);

    let read = read_snapshot(&storage, "HOJ", "1166", "C++");
    assert_eq!(read.as_deref(), Some(code));

}

#[test]
fn read_falls_back_to_scan_when_language_changed() {
    // 提交时是 "C++"，查询时详情返回别的写法（服务端归一 / 选手改语言）：
    // 精确路径落空，但快照就在那儿，必须能扫出来
    let (storage, _dir) = temp_storage("fallback");
    let code = "#include <bits/stdc++.h>";

    write_snapshot(&storage, "HOJ", "1166", "C++", code);

    let read = read_snapshot(&storage, "HOJ", "1166", "C++ 17");
    assert_eq!(read.as_deref(), Some(code));

}

#[test]
fn read_returns_none_when_absent() {
    let (storage, _dir) = temp_storage("absent");
    assert_eq!(read_snapshot(&storage, "HOJ", "999999", "C++"), None);
}

#[test]
fn snapshots_of_different_ojs_do_not_collide() {
    // submit_id 是各 OJ 自增的资源号，跨 OJ 必然重号
    let (storage, _dir) = temp_storage("oj-dimension");

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

}

#[test]
fn write_snapshot_failure_does_not_panic() {
    // best-effort 语义：非法 submit_id 只记录告警，不 panic、不阻断提交
    let (storage, _dir) = temp_storage("best-effort");
    write_snapshot(&storage, "HOJ", "../evil", "C++", "code");
    // 未写出任何文件
    assert!(!storage.exists("submissions"));
}

// ── 占用统计与过期清理（设置页「清理本地数据」） ──

/// 把文件 mtime 改成 `days_ago` 天前，用于构造「过期留档」。
///
/// 用 std 的 `File::set_times`（Rust 1.75+）而不是引入 `filetime` 依赖：为一条测试
/// 加一个第三方依赖不划算。
fn age_file(path: &std::path::Path, days_ago: u64) {
    let when = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(days_ago * 24 * 60 * 60))
        .expect("时间回拨失败");
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("打开待改时间的留档失败")
        .set_times(std::fs::FileTimes::new().set_modified(when))
        .expect("设置 mtime 失败");
}

#[test]
fn inspect_snapshots_counts_totals_and_stale() {
    let (storage, dir) = temp_storage("usage");
    write_snapshot(&storage, "HOJ", "1167", "C++", "int main(){}");
    write_snapshot(&storage, "HOJ", "1168", "C++", "int main(){return 0;}");
    write_snapshot(&storage, "Hydro", "42", "Python3", "print(1)");
    age_file(&dir.join("submissions/HOJ/1168.cpp"), 60);
    age_file(&dir.join("submissions/Hydro/42.py"), 60);

    let usage = inspect_snapshots(&storage, SNAPSHOT_KEEP_DAYS);
    assert_eq!(usage.total_count, 3, "应统计全部留档（跨 OJ）");
    assert!(usage.total_bytes > 0);
    assert_eq!(usage.stale_count, 2, "60 天前的留档应算过期（窗口 30 天）");
    assert!(usage.stale_bytes < usage.total_bytes, "过期只占一部分");

    // 窗口放宽到 365 天：同一批留档都不再过期（判据确实跟着窗口走）
    assert_eq!(inspect_snapshots(&storage, 365).stale_count, 0);

    // 统计是只读的：不改变磁盘状态
    assert_eq!(inspect_snapshots(&storage, SNAPSHOT_KEEP_DAYS).total_count, 3);

}

#[test]
fn purge_stale_snapshots_keeps_fresh_and_removes_aged() {
    let (storage, dir) = temp_storage("purge-mixed");
    write_snapshot(&storage, "HOJ", "1167", "C++", "int main(){}");
    write_snapshot(&storage, "HOJ", "1168", "C++", "int main(){return 0;}");
    write_snapshot(&storage, "Hydro", "42", "Python3", "print(1)");
    age_file(&dir.join("submissions/HOJ/1168.cpp"), 60);
    age_file(&dir.join("submissions/Hydro/42.py"), 60);

    let expected = inspect_snapshots(&storage, SNAPSHOT_KEEP_DAYS);
    let (removed, freed) = purge_stale_snapshots(&storage, SNAPSHOT_KEEP_DAYS);

    assert_eq!(removed, 2);
    assert_eq!(freed, expected.stale_bytes, "释放字节数须与预览一致");
    // 核心约束：按时间保留不能变成「一刀切删光」
    assert!(
        storage.exists("submissions/HOJ/1167.cpp"),
        "窗口内的留档不得被删 —— 这是本功能唯一会丢数据的地方"
    );
    assert!(!storage.exists("submissions/HOJ/1168.cpp"));
    // 空掉的 OJ 子目录回收，仍有留档的保留
    assert!(!dir.join("submissions").join("Hydro").exists(), "清空的 OJ 子目录应被回收");
    assert!(dir.join("submissions").join("HOJ").exists(), "仍有留档的目录不得回收");

    assert_eq!(inspect_snapshots(&storage, SNAPSHOT_KEEP_DAYS).total_count, 1);

}

#[test]
fn inspect_and_purge_are_safe_without_any_snapshot() {
    // 从未提交过（目录不存在）：统计返回全零、清理是 no-op 而不是报错
    let (storage, _dir) = temp_storage("purge-empty");
    assert_eq!(inspect_snapshots(&storage, SNAPSHOT_KEEP_DAYS), SnapshotUsage::default());
    assert_eq!(purge_stale_snapshots(&storage, SNAPSHOT_KEEP_DAYS), (0, 0));
}
