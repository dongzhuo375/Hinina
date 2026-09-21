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
