use super::*;
use std::path::PathBuf;

fn storage() -> Storage {
    Storage::new(PathBuf::from("/tmp/hinina-test"))
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
