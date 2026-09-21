use super::*;
use crate::test_support::TempDir;

fn storage() -> Storage {
    Storage::new(std::env::temp_dir().join("hinina-test"))
}

#[test]
fn resolve_normal_path() {
    let s = storage();
    assert!(s.resolve("foo/bar/baz").is_ok());
    assert!(s.resolve("a").is_ok());
    assert!(s.resolve("a/b/./c").is_ok());
}

#[test]
fn resolve_rejects_parent_dir() {
    let s = storage();
    assert!(s.resolve("..").is_err());
    assert!(s.resolve("../etc").is_err());
    assert!(s.resolve("foo/../../../etc/passwd").is_err());
    assert!(s.resolve("a/../b/../c").is_err());
}

#[test]
fn resolve_deep_nested() {
    let s = storage();
    assert!(s.resolve("a/b/c/d/e/f").is_ok());
}

/// 原子写：新建文件内容正确，且不残留 `.tmp` 临时文件。
#[test]
fn write_string_atomic_creates_file_without_tmp_leftover() {
    let dir = TempDir::named("hinina-storage-atomic-create");
    let s = Storage::new(dir.to_path_buf());

    s.write_string_atomic("sessions/HOJ.json", "{\"token\":\"t-1\"}").unwrap();

    assert_eq!(
        s.read_to_string("sessions/HOJ.json").unwrap(),
        "{\"token\":\"t-1\"}"
    );
    let entries = s.list("sessions").unwrap();
    assert_eq!(entries.len(), 1, "不应残留临时文件: {entries:?}");
}

/// 原子写覆盖已有文件：内容完整替换（不是追加 / 交错的半截）。
#[test]
fn write_string_atomic_replaces_existing_content() {
    let dir = TempDir::named("hinina-storage-atomic-replace");
    let s = Storage::new(dir.to_path_buf());

    s.write_string_atomic("sessions/HOJ.json", "{\"token\":\"old\"}").unwrap();
    s.write_string_atomic("sessions/HOJ.json", "{\"token\":\"new-and-longer\"}").unwrap();

    assert_eq!(
        s.read_to_string("sessions/HOJ.json").unwrap(),
        "{\"token\":\"new-and-longer\"}"
    );
    let entries = s.list("sessions").unwrap();
    assert_eq!(entries.len(), 1, "覆盖后不应残留临时文件: {entries:?}");
}

/// 原子写拒绝越权路径（与普通写同一道防线）。
#[test]
fn write_string_atomic_rejects_parent_dir() {
    let dir = TempDir::named("hinina-storage-atomic-escape");
    let s = Storage::new(dir.to_path_buf());

    assert!(s.write_string_atomic("../escape.json", "x").is_err());
}

/// 原子写前清理同目标的崩溃残片：`{target}.{数字}.tmp` 被删除，
/// 其他文件（含相似命名）不误伤。
#[test]
fn write_string_atomic_cleans_stale_temps_of_same_target() {
    let dir = TempDir::named("hinina-storage-atomic-stale");
    let s = Storage::new(dir.to_path_buf());
    s.create_dir("sessions").unwrap();

    // 模拟上次崩溃残留：同目标两个残片 + 一个相似但非本约定的文件
    std::fs::write(dir.join("sessions").join("HOJ.json.41.tmp"), "stale-1").unwrap();
    std::fs::write(dir.join("sessions").join("HOJ.json.999999.tmp"), "stale-2").unwrap();
    std::fs::write(dir.join("sessions").join("HOJ.json.tmp"), "not-ours").unwrap();
    std::fs::write(dir.join("sessions").join("other.json.7.tmp"), "other-target").unwrap();

    s.write_string_atomic("sessions/HOJ.json", "{\"token\":\"t\"}").unwrap();

    let remaining: Vec<String> = std::fs::read_dir(dir.join("sessions"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    // 目标文件存在；同目标的数字残片被清掉；非约定命名与其他目标的文件保留
    assert!(remaining.contains(&"HOJ.json".to_string()), "目标文件应存在: {remaining:?}");
    assert!(!remaining.contains(&"HOJ.json.41.tmp".to_string()), "同目标残片应被清理: {remaining:?}");
    assert!(!remaining.contains(&"HOJ.json.999999.tmp".to_string()), "同目标残片应被清理: {remaining:?}");
    assert!(remaining.contains(&"HOJ.json.tmp".to_string()), "非约定命名不误伤");
    assert!(remaining.contains(&"other.json.7.tmp".to_string()), "其他目标不误伤");
}
