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
    assert!(s.resolve("foo/bar/").is_ok());
}

/// 空路径 / 纯点路径会解析为 `base_dir` 本身或其子目录：`remove_all("")`
/// 足以删掉数据根目录，`remove_all("workspaces/.")` 足以删掉全部工作区。
/// 防线放在 `resolve`（所有 Storage 调用的公共入口），不靠调用方自律。
#[test]
fn resolve_rejects_empty_or_dot_only_paths() {
    let s = storage();
    assert!(s.resolve("").is_err());
    assert!(s.resolve(".").is_err());
    assert!(s.resolve("workspaces/.").is_err());
    // 内嵌 `./` 同样拒绝：白名单只放行 Normal，规则简单可穷举
    assert!(s.resolve("a/b/./c").is_err());
}

#[test]
fn remove_all_rejects_empty_path() {
    let dir = TempDir::named("hinina-storage-empty-remove");
    std::fs::create_dir_all(dir.path()).unwrap();
    std::fs::write(dir.join("keep.txt"), "x").unwrap();
    let s = Storage::new(dir.to_path_buf());

    assert!(s.remove_all("").is_err(), "空路径不得删到数据根目录");
    assert!(s.remove_all(".").is_err());
    assert!(dir.join("keep.txt").exists(), "数据必须原样保留");
}

#[test]
fn resolve_rejects_parent_dir() {
    let s = storage();
    assert!(s.resolve("..").is_err());
    assert!(s.resolve("../etc").is_err());
    assert!(s.resolve("foo/../../../etc/passwd").is_err());
    assert!(s.resolve("a/../b/../c").is_err());
}

/// 绝对路径形态（根目录 / Windows 盘符前缀）会在 `join` 时**整体替换 base_dir**，
/// 必须在 `resolve` 就拒绝 —— 防线不能单点压在 `fs_workspace_repo` 的白名单上。
#[test]
fn resolve_rejects_absolute_paths() {
    let s = storage();
    // RootDir：`/x` 在 Windows 上是「当前盘根」，同样会替换 base
    assert!(s.resolve("/evil").is_err());
    assert!(s.resolve("/etc/passwd").is_err());
}

/// Windows 盘符前缀（`Prefix` 组件）：`C:\x`（带根）与 `C:x`（盘符相对路径）
/// 都会让 `join` 逃出 base_dir；verbatim（`\\?\C:\...`）同拒。
#[cfg(windows)]
#[test]
fn resolve_rejects_windows_prefix_paths() {
    let s = storage();
    assert!(s.resolve("C:\\evil").is_err());
    assert!(s.resolve("C:evil").is_err());
    assert!(s.resolve("\\\\?\\C:\\evil").is_err());
}

/// 原子写对绝对路径形态同样拒绝（与普通写同一道 `resolve` 防线）。
#[test]
fn write_string_atomic_rejects_absolute_paths() {
    let dir = TempDir::named("hinina-storage-atomic-absolute");
    let s = Storage::new(dir.to_path_buf());

    assert!(s.write_string_atomic("/evil.json", "x").is_err());
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

    s.write_string_atomic("sessions/HOJ.json", "{\"token\":\"t-1\"}")
        .unwrap();

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

    s.write_string_atomic("sessions/HOJ.json", "{\"token\":\"old\"}")
        .unwrap();
    s.write_string_atomic("sessions/HOJ.json", "{\"token\":\"new-and-longer\"}")
        .unwrap();

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
    std::fs::write(
        dir.join("sessions").join("other.json.7.tmp"),
        "other-target",
    )
    .unwrap();

    s.write_string_atomic("sessions/HOJ.json", "{\"token\":\"t\"}")
        .unwrap();

    let remaining: Vec<String> = std::fs::read_dir(dir.join("sessions"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    // 目标文件存在；同目标的数字残片被清掉；非约定命名与其他目标的文件保留
    assert!(
        remaining.contains(&"HOJ.json".to_string()),
        "目标文件应存在: {remaining:?}"
    );
    assert!(
        !remaining.contains(&"HOJ.json.41.tmp".to_string()),
        "同目标残片应被清理: {remaining:?}"
    );
    assert!(
        !remaining.contains(&"HOJ.json.999999.tmp".to_string()),
        "同目标残片应被清理: {remaining:?}"
    );
    assert!(
        remaining.contains(&"HOJ.json.tmp".to_string()),
        "非约定命名不误伤"
    );
    assert!(
        remaining.contains(&"other.json.7.tmp".to_string()),
        "其他目标不误伤"
    );
}

/// symlink_metadata：存在文件返回元数据，缺失/穿越路径报 Io 错误。
#[test]
fn symlink_metadata_reads_regular_files_and_rejects_invalid_paths() {
    let dir = TempDir::named("hinina-storage-lstat");
    let s = Storage::new(dir.to_path_buf());
    s.write_string("sessions/HOJ.json", "{\"token\":\"t\"}")
        .unwrap();

    let meta = s.symlink_metadata("sessions/HOJ.json").unwrap();
    assert!(meta.is_file());
    assert_eq!(meta.len(), 13);
    assert!(meta.modified().is_ok());

    assert!(s.symlink_metadata("sessions/missing.json").is_err());
    assert!(s.symlink_metadata("../evil").is_err());
}

/// lstat 语义：链接本身是「符号链接」而非文件，绝不返回目标文件的元数据。
/// （Unix-only：Windows 建符号链接需要开发者模式/管理员权限，CI 不具备。）
#[cfg(unix)]
#[test]
fn symlink_metadata_does_not_follow_symlink_targets() {
    use std::os::unix::fs::symlink;

    let dir = TempDir::named("hinina-storage-lstat-link");
    let external = TempDir::named("hinina-storage-lstat-target");
    let s = Storage::new(dir.to_path_buf());
    std::fs::create_dir_all(external.path()).unwrap();
    std::fs::write(external.join("outside.json"), "outside").unwrap();
    symlink(external.join("outside.json"), dir.join("link.json")).unwrap();

    let meta = s.symlink_metadata("link.json").unwrap();
    assert!(meta.file_type().is_symlink(), "必须报告链接本身");
    assert!(!meta.is_file(), "不得把链接目标当成文件");
}

/// `remove_empty_dir`：只删真实空目录——文件与非空目录报错（文件绝不被删）。
#[test]
fn remove_empty_dir_only_removes_real_empty_directories() {
    let dir = TempDir::named("hinina-storage-remove-empty-dir");
    let s = Storage::new(dir.to_path_buf());
    s.create_dir("empty").unwrap();
    s.write_string("full/a.txt", "x").unwrap();

    assert!(s.remove_empty_dir("full").is_err(), "非空目录必须失败");
    assert!(
        s.remove_empty_dir("full/a.txt").is_err(),
        "文件必须失败（目录专用删除不得删文件）"
    );
    assert!(s.exists("full/a.txt"));
    assert!(s.remove_empty_dir("empty").is_ok());
    assert!(!s.exists("empty"));
}

/// 符号链接不被跟随：对链接到目录的条目 `remove_empty_dir` 必须失败且链接与
/// 目标原样保留（Unix 的 `rmdir` 对 symlink 返回 ENOTDIR）。
#[cfg(unix)]
#[test]
fn remove_empty_dir_does_not_follow_symlinks() {
    use std::os::unix::fs::symlink;
    let dir = TempDir::named("hinina-storage-remove-empty-link");
    let s = Storage::new(dir.to_path_buf());
    let target = TempDir::named("hinina-storage-remove-empty-link-target");
    std::fs::create_dir_all(target.path()).unwrap();
    symlink(target.path(), dir.join("link")).unwrap();

    assert!(
        s.remove_empty_dir("link").is_err(),
        "链接不得被当作空目录删除"
    );
    assert!(
        std::fs::symlink_metadata(dir.join("link")).is_ok(),
        "链接应保留"
    );
    assert!(target.path().exists(), "目标目录应保留");
}

/// `remove_file`：只删文件——目录报错（目录绝不被删）；Unix 上对符号链接删除
/// 链接本身、目标原样保留（unlink 不跟随）。
#[test]
fn remove_file_only_removes_files() {
    let dir = TempDir::named("hinina-storage-remove-file");
    let s = Storage::new(dir.to_path_buf());
    s.write_string("a.txt", "x").unwrap();
    s.create_dir("sub").unwrap();

    assert!(s.remove_file("a.txt").is_ok());
    assert!(!s.exists("a.txt"));
    assert!(s.remove_file("sub").is_err(), "目录不得被文件专用删除删掉");
    assert!(s.exists("sub"));
}

#[cfg(unix)]
#[test]
fn remove_file_on_symlink_removes_the_link_not_the_target() {
    use std::os::unix::fs::symlink;
    let dir = TempDir::named("hinina-storage-remove-file-link");
    let s = Storage::new(dir.to_path_buf());
    let target = TempDir::named("hinina-storage-remove-file-link-target");
    // TempDir 只清残留、不建目录（82191da 同款教训）——写目标文件前必须先建
    std::fs::create_dir_all(target.path()).unwrap();
    let target_file = target.path().join("secret.txt");
    std::fs::write(&target_file, "outside").unwrap();
    symlink(&target_file, dir.join("link.txt")).unwrap();

    assert!(s.remove_file("link.txt").is_ok(), "删除的是链接本身");
    assert!(!dir.join("link.txt").exists());
    assert!(target_file.exists(), "目标文件必须保留");
}
