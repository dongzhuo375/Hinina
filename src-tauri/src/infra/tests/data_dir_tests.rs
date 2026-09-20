// infra/data_dir.rs 单元测试：指针读写、路径校验、目录可用性、迁移与启动准备。
//
// 全部基于独立临时目录（每次一个，避免并行测试相互干扰），不触碰真实
// `%LOCALAPPDATA%` / `%TEMP%/hinina`。

use super::*;

use std::sync::atomic::{AtomicUsize, Ordering};

/// 独立临时目录（每次调用一个）。
fn unique_dir(tag: &str) -> PathBuf {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "hinina-test-datadir-{}-{}-{}",
        tag,
        std::process::id(),
        SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// 在目录下造一个文件（自动建父目录）。
fn write_file(dir: &Path, rel: &str, content: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

/// 造一个「有数据的旧目录」（覆盖迁移清单里的若干项 + 必须不搬的 logs）。
fn seed_legacy(dir: &Path) {
    write_file(dir, "config.json", r#"{"oj":{"active":"HOJ"}}"#);
    write_file(dir, "workspaces/HOJ-1012-1000/main.cpp", "int main(){}");
    write_file(dir, "submissions/HOJ/1167.cpp", "int main(){}");
    write_file(dir, "announcements_read/1012_uid-1.json", r#"{"readIds":["9001"]}"#);
    write_file(dir, "logs/hinina.log", "old log line");
}

/// 构造指针（只关心 data_dir / migrate_from 的用例用它，避免逐处写全字段）。
fn pointer(data_dir: Option<&Path>, migrate_from: Option<&Path>) -> DataDirPointer {
    DataDirPointer {
        data_dir: data_dir.map(|p| p.to_string_lossy().into_owned()),
        migrate_from: migrate_from.map(|p| p.to_string_lossy().into_owned()),
        legacy_migrated: None,
    }
}

/// 构造带 `legacy_migrated` 的指针（三态语义的用例用它）。
fn pointer_with_legacy(data_dir: Option<&Path>, legacy_migrated: Option<bool>) -> DataDirPointer {
    DataDirPointer {
        data_dir: data_dir.map(|p| p.to_string_lossy().into_owned()),
        migrate_from: None,
        legacy_migrated,
    }
}

// ── 指针读写 ──

#[test]
fn pointer_round_trip() {
    let dir = unique_dir("pointer");
    let pointer = DataDirPointer {
        data_dir: Some(r"D:\data\hinina".into()),
        migrate_from: Some(r"C:\tmp\hinina".into()),
        legacy_migrated: Some(true),
    };
    write_pointer(&dir, &pointer).expect("写入指针失败");

    assert_eq!(read_pointer(&dir), pointer, "指针应原样读回");
    // 指针文件固定放在默认目录下（否则「自定义目录在哪」本身就需要指针）
    assert!(pointer_path(&dir).exists());
    assert_eq!(pointer_path(&dir), dir.join("data_dir.json"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_or_corrupt_pointer_degrades_to_default() {
    let dir = unique_dir("pointer-bad");

    // 文件不存在 → 默认（未指定）
    assert_eq!(read_pointer(&dir), DataDirPointer::default());

    // 损坏 → 仍按未指定处理，绝不阻断启动
    write_file(&dir, "data_dir.json", "not-json{{{");
    assert_eq!(read_pointer(&dir), DataDirPointer::default());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pointer_omits_absent_fields() {
    // skip_serializing_if：未指定目录时文件里不该出现 null 字段
    let dir = unique_dir("pointer-omit");
    write_pointer(&dir, &DataDirPointer::default()).expect("写入指针失败");
    let raw = std::fs::read_to_string(pointer_path(&dir)).unwrap();
    assert_eq!(raw.trim(), "{}");
    let _ = std::fs::remove_dir_all(&dir);
}

// ── 目录可用性与空判定 ──

#[test]
fn dir_is_usable_creates_missing_dir_and_leaves_no_probe() {
    let root = unique_dir("usable");
    let dir = root.join("nested");

    assert!(dir_is_usable(&dir), "应能创建并写入");
    assert!(dir.is_dir());
    // 探针文件必须被清掉，否则「空目录」判定会失败
    assert!(dir_is_empty(&dir), "探测后不应留下残留文件");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn dir_is_usable_rejects_file_path() {
    let dir = unique_dir("usable-file");
    write_file(&dir, "afile", "x");
    // 目标是文件 → 无法作为目录
    assert!(!dir_is_usable(&dir.join("afile")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dir_is_empty_distinguishes_missing_empty_and_nonempty() {
    let dir = unique_dir("empty");
    assert!(dir_is_empty(&dir), "不存在视为空");
    std::fs::create_dir_all(&dir).unwrap();
    assert!(dir_is_empty(&dir), "空目录");
    write_file(&dir, "x.txt", "1");
    assert!(!dir_is_empty(&dir), "有内容");
    let _ = std::fs::remove_dir_all(&dir);
}

// ── 目标目录校验（设置页「更改目录」） ──

#[test]
fn validate_target_accepts_fresh_absolute_dir() {
    let legacy = unique_dir("validate-legacy");
    let target = unique_dir("validate-ok");

    let got = validate_target(&target, &legacy).expect("全新绝对路径应通过");
    assert_eq!(got, target);
    assert!(target.is_dir(), "校验顺带创建目录");

    let _ = std::fs::remove_dir_all(&target);
    let _ = std::fs::remove_dir_all(&legacy);
}

#[test]
fn validate_target_rejects_relative_path() {
    let legacy = unique_dir("validate-rel-legacy");
    let err = validate_target(Path::new("relative/dir"), &legacy).expect_err("相对路径应被拒");
    assert!(matches!(err, AppError::Config(_)), "应为配置类错误");
    assert!(err.to_string().contains("绝对路径"));
}

#[test]
fn validate_target_rejects_legacy_temp_dir_and_its_children() {
    let legacy = unique_dir("validate-legacy-guard");

    // 旧临时目录本身：正是本次要修的问题，必须拒绝
    let err = validate_target(&legacy, &legacy).expect_err("指向临时目录应被拒");
    assert!(err.to_string().contains("临时目录"), "错误信息应说明原因");

    // 其子目录同样拒绝（否则数据又落在会被清理的位置）
    let child = legacy.join("sub");
    std::fs::create_dir_all(&child).unwrap();
    assert!(validate_target(&child, &legacy).is_err(), "临时目录的子目录也应被拒");

    let _ = std::fs::remove_dir_all(&legacy);
}

#[test]
fn validate_target_rejects_file() {
    let legacy = unique_dir("validate-file-legacy");
    let dir = unique_dir("validate-file");
    write_file(&dir, "afile", "x");

    let err = validate_target(&dir.join("afile"), &legacy).expect_err("文件应被拒");
    assert!(err.to_string().contains("不是目录"));

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&legacy);
}

#[test]
fn validate_target_rejects_non_empty_dir() {
    // 决策：拒绝非空目录 —— 覆盖会丢数据、合并会混入别人的配置
    let legacy = unique_dir("validate-nonempty-legacy");
    let dir = unique_dir("validate-nonempty");
    write_file(&dir, "existing.json", "{}");

    let err = validate_target(&dir, &legacy).expect_err("非空目录应被拒");
    assert!(err.to_string().contains("已有内容"), "错误信息应给出可操作提示");

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&legacy);
}

#[test]
fn validate_target_rejects_empty_string() {
    let legacy = unique_dir("validate-empty-legacy");
    assert!(validate_target(Path::new(""), &legacy).is_err());
}

// ── 迁移 ──

#[test]
fn migrate_moves_listed_entries_and_skips_logs() {
    let from = unique_dir("migrate-from");
    let to = unique_dir("migrate-to");
    seed_legacy(&from);

    let outcome = migrate(&from, &to);

    assert!(outcome.is_ok(), "不应有失败项: {:?}", outcome.failed);
    assert_eq!(
        outcome.moved,
        vec![
            "config.json".to_string(),
            "workspaces".to_string(),
            "submissions".to_string(),
            "announcements_read".to_string(),
        ],
        "应按 MIGRATED_ENTRIES 的顺序搬运（logs 不在清单内，故不出现）"
    );

    // 内容确实搬过去了
    assert_eq!(
        std::fs::read_to_string(to.join("workspaces/HOJ-1012-1000/main.cpp")).unwrap(),
        "int main(){}"
    );
    assert!(to.join("config.json").exists());
    assert!(to.join("submissions/HOJ/1167.cpp").exists());
    assert!(to.join("announcements_read/1012_uid-1.json").exists());

    // logs 不搬：旧日志留在原地，新目录没有 logs
    assert!(!to.join("logs").exists(), "logs 按约定不迁移");

    let _ = std::fs::remove_dir_all(&from);
    let _ = std::fs::remove_dir_all(&to);
}

#[test]
fn migrate_keeps_legacy_dir_when_logs_remain() {
    let from = unique_dir("migrate-keep-from");
    let to = unique_dir("migrate-keep-to");
    seed_legacy(&from);

    let outcome = migrate(&from, &to);

    // logs/ 未搬 → 旧目录非空，删不掉，如实回报 false（属预期，不是失败）
    assert!(!outcome.legacy_removed, "留着 logs 时旧目录无法删除");
    assert!(from.join("logs/hinina.log").exists(), "旧日志原地保留");

    let _ = std::fs::remove_dir_all(&from);
    let _ = std::fs::remove_dir_all(&to);
}

#[test]
fn migrate_removes_legacy_dir_when_nothing_left() {
    let from = unique_dir("migrate-full-from");
    let to = unique_dir("migrate-full-to");
    write_file(&from, "config.json", "{}");
    write_file(&from, "workspaces/w/main.cpp", "x");

    let outcome = migrate(&from, &to);

    assert!(outcome.is_ok());
    assert!(outcome.legacy_removed, "搬空后旧目录应被删除");
    assert!(!from.exists());

    let _ = std::fs::remove_dir_all(&to);
}

#[test]
fn migrate_skips_entries_already_present_at_target() {
    // 不覆盖既有数据：重试场景下这是常态，不是错误
    let from = unique_dir("migrate-skip-from");
    let to = unique_dir("migrate-skip-to");
    write_file(&from, "config.json", r#"{"old":true}"#);
    write_file(&to, "config.json", r#"{"new":true}"#);

    let outcome = migrate(&from, &to);

    assert_eq!(outcome.skipped, vec!["config.json".to_string()]);
    assert!(outcome.moved.is_empty());
    assert!(outcome.is_ok(), "跳过不是失败");
    assert_eq!(
        std::fs::read_to_string(to.join("config.json")).unwrap(),
        r#"{"new":true}"#,
        "目标既有数据不得被覆盖"
    );

    let _ = std::fs::remove_dir_all(&from);
    let _ = std::fs::remove_dir_all(&to);
}

#[test]
fn migrate_is_noop_when_source_has_nothing_listed() {
    let from = unique_dir("migrate-empty-from");
    let to = unique_dir("migrate-empty-to");
    write_file(&from, "logs/hinina.log", "only logs");

    let outcome = migrate(&from, &to);

    assert!(outcome.is_noop(), "只有 logs 时无事可做: {:?}", outcome);

    let _ = std::fs::remove_dir_all(&from);
    let _ = std::fs::remove_dir_all(&to);
}

#[test]
fn migrate_never_deletes_unmigrated_entries() {
    // **这是本模块最危险的一条路径**：旧目录清理若用递归删除，会把「迁移失败的条目」
    // 一起删掉 —— 用户以为数据搬过去了，实际被删了。故只允许删空目录。
    //
    // 构造方式：让目标目录不可用（`to` 本身是个文件）→ 所有条目都搬不过去 →
    // 旧目录必须**完整保留**。
    let from = unique_dir("migrate-safe-from");
    seed_legacy(&from);
    let to = unique_dir("migrate-safe-to");
    write_file(&to, "blocker", "i am a file");
    let blocked_target = to.join("blocker");

    let outcome = migrate(&from, &blocked_target);

    assert!(!outcome.is_ok(), "目标不可用必须记录为失败");
    assert!(outcome.moved.is_empty(), "一条都不该被搬走");
    // 核心断言：源数据一条都不能少
    assert!(from.join("config.json").exists(), "未迁移的 config.json 必须保留");
    assert!(
        from.join("workspaces/HOJ-1012-1000/main.cpp").exists(),
        "未迁移的代码必须保留 —— 递归删除会让它消失"
    );
    assert!(!outcome.legacy_removed, "有失败项时绝不能删除旧目录");

    let _ = std::fs::remove_dir_all(&from);
    let _ = std::fs::remove_dir_all(&to);
}

#[test]
fn existing_target_is_skipped_not_failed() {
    // 「目标已存在 → 跳过（不覆盖）」是刻意的（重试幂等），**跳过不算失败**。
    // 注意这与「半拷贝」的区别：修复后 `dst.exists()` 只可能是完整搬运的结果
    // （复制走暂存名 + 改名），故无需区分「完整」与「半截」。
    let from = unique_dir("skip-from");
    let to = unique_dir("skip-to");
    write_file(&from, "config.json", "{}");
    write_file(&from, "workspaces/w/main.cpp", "int main(){}");
    write_file(&to, "workspaces/existing.txt", "already here");

    let outcome = migrate(&from, &to);

    assert!(outcome.is_ok(), "跳过不是失败: {:?}", outcome);
    assert_eq!(outcome.skipped, vec!["workspaces".to_string()]);
    assert!(outcome.moved.contains(&"config.json".to_string()));
    assert!(
        to.join("workspaces/existing.txt").exists(),
        "目标既有内容不得被覆盖"
    );

    let _ = std::fs::remove_dir_all(&from);
    let _ = std::fs::remove_dir_all(&to);
}

#[test]
fn mark_legacy_migrated_persists_failure_state_and_survives_pointer_rewrite() {
    // 缺陷回归（测试抓到）：`mark_legacy_migrated` 曾用启动时读到的**陈旧指针副本**
    // 写入，会把同一次启动里刚被 `clear_migrate_from` 清掉的字段复活。
    // 现在改为读-改-写，两处修改必须互不干扰。
    let dir = unique_dir("pointer-rmw");
    write_pointer(&dir, &pointer(Some(Path::new(r"D:\custom")), Some(Path::new(r"C:\from"))))
        .expect("写指针失败");

    // 先清 migrate_from（模拟分支 ① 成功）
    clear_migrate_from(&dir);
    // 再写搬家失败标记（模拟分支 ② 失败）
    mark_legacy_migrated(&dir, false);

    let after = read_pointer(&dir);
    assert_eq!(after.migrate_from, None, "清掉的字段不得被后一次写入复活");
    assert_eq!(after.legacy_migrated, Some(false), "失败态必须落盘");
    assert_eq!(
        after.data_dir,
        Some(r"D:\custom".to_string()),
        "用户指定的目录不得被抹掉"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn should_attempt_legacy_retries_failed_migration_regardless_of_source() {
    // **MEDIUM 缺陷回归**：临时目录搬家失败后用户改用自定义目录，若不重试，
    // 那份数据会永远留在会被系统清理的临时目录里。
    let legacy = unique_dir("attempt-legacy");
    write_file(&legacy, "config.json", "{}");

    let custom = unique_dir("attempt-custom");
    let default = unique_dir("attempt-default");

    // 从未尝试 + 自定义目录 → 不搬（尊重用户选择）
    assert!(!should_attempt_legacy(
        &pointer(None, None),
        DataDirSource::Custom,
        &legacy
    ));
    // 从未尝试 + 默认目录 → 搬（全新升级场景）
    assert!(should_attempt_legacy(
        &pointer(None, None),
        DataDirSource::Default,
        &legacy
    ));
    // **试过但失败 + 自定义目录 → 仍要搬**（本缺陷的核心）
    assert!(should_attempt_legacy(
        &pointer_with_legacy(None, Some(false)),
        DataDirSource::Custom,
        &legacy
    ));
    // 已成功 + 任意目录 → 永不重试（否则用户删掉的旧工作区会复活）
    assert!(!should_attempt_legacy(
        &pointer_with_legacy(None, Some(true)),
        DataDirSource::Default,
        &legacy
    ));
    // 旧目录不存在 → 无事可做
    assert!(!should_attempt_legacy(
        &pointer(None, None),
        DataDirSource::Default,
        &default.join("nope")
    ));

    let _ = std::fs::remove_dir_all(&legacy);
    let _ = std::fs::remove_dir_all(&custom);
    let _ = std::fs::remove_dir_all(&default);
}

#[test]
fn prepare_startup_records_failure_so_later_retry_is_not_lost() {
    // MEDIUM 缺陷的**端到端**部分：失败必须落成 `Some(false)`，且此后即使来源换成
    // 自定义目录也仍会重试（`should_attempt_legacy` 的端到端体现）。
    //
    // 「让迁移真失败」在单测里无法确定性构造（需要文件被别的进程占用），故这里
    // 直接用 `mark_legacy_migrated(false)` 落到盘上，再验证下一次启动确实重试。
    let default_dir = unique_dir("retry-e2e-default");
    let legacy = unique_dir("retry-e2e-legacy");
    seed_legacy(&legacy);
    let custom = unique_dir("retry-e2e-custom");
    std::fs::create_dir_all(&custom).unwrap();

    // 模拟「上次尝试过且失败」+ 用户已改用自定义目录
    write_pointer(
        &default_dir,
        &DataDirPointer {
            data_dir: Some(custom.to_string_lossy().into_owned()),
            migrate_from: None,
            legacy_migrated: Some(false),
        },
    )
    .expect("写指针失败");

    let (plan, outcome) = prepare_startup(&default_dir, &legacy);

    assert_eq!(plan.base_dir, custom);
    assert!(outcome.is_ok(), "{:?}", outcome);
    assert!(
        custom.join("workspaces/HOJ-1012-1000/main.cpp").exists(),
        "**失败过的旧数据必须被搬到自定义目录** —— 不搬就是 MEDIUM 缺陷（滞留临时目录）"
    );
    assert_eq!(
        read_pointer(&default_dir).legacy_migrated,
        Some(true),
        "成功后置位一次性标记"
    );

    let _ = std::fs::remove_dir_all(&default_dir);
    let _ = std::fs::remove_dir_all(&legacy);
    let _ = std::fs::remove_dir_all(&custom);
}

#[test]
fn prepare_startup_runs_both_migration_sources_in_one_launch() {
    // ① 显式迁移与 ② 临时目录搬家**同一次启动内都可能执行** —— 曾经 ① 成功后
    // 直接 return，② 被跳过（这正是上一个缺陷的成因）。
    let default_dir = unique_dir("both-default");
    let source = unique_dir("both-source");
    let legacy = unique_dir("both-legacy");
    seed_legacy(&source);
    seed_legacy(&legacy);

    let target = unique_dir("both-target");
    std::fs::create_dir_all(&target).unwrap();
    // 指针：改到 target 并迁移 source；legacy 标记为「试过但失败」
    write_pointer(
        &default_dir,
        &DataDirPointer {
            data_dir: Some(target.to_string_lossy().into_owned()),
            migrate_from: Some(source.to_string_lossy().into_owned()),
            legacy_migrated: Some(false),
        },
    )
    .expect("写指针失败");

    let (plan, outcome) = prepare_startup(&default_dir, &legacy);

    assert_eq!(plan.base_dir, target);
    assert!(outcome.is_ok(), "两个来源都应成功: {:?}", outcome);
    assert!(
        target.join("workspaces/HOJ-1012-1000/main.cpp").exists(),
        "两个来源的内容都应落到目标目录"
    );
    assert_eq!(
        read_pointer(&default_dir).legacy_migrated,
        Some(true),
        "临时目录搬家应在同一次启动内完成"
    );

    let _ = std::fs::remove_dir_all(&default_dir);
    let _ = std::fs::remove_dir_all(&source);
    let _ = std::fs::remove_dir_all(&legacy);
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn pointer_write_is_atomic_and_leaves_no_temp_file() {
    // 原子写（temp + rename）：写完不得留下 `.json.tmp` 残片，且内容可读回
    let dir = unique_dir("pointer-atomic");
    write_pointer(&dir, &pointer(Some(Path::new(r"D:\x")), None)).expect("写入失败");

    assert!(pointer_path(&dir).exists());
    assert!(
        !pointer_path(&dir).with_extension("json.tmp").exists(),
        "不得留下临时文件残片"
    );
    assert_eq!(
        read_pointer(&dir).data_dir,
        Some(r"D:\x".to_string()),
        "内容应可完整读回"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn failed_move_leaves_nothing_behind_so_retry_actually_retries() {
    // **HIGH 缺陷回归**：跨卷复制中途失败若留下半拷贝目标，重试逻辑用 `dst.exists()`
    // 判「已完成」→ 半拷贝被误判成「已跳过」→ 失败计数为零 → 迁移标记被清除 →
    // 重试自解除，数据目录永久残缺。
    //
    // 本用例断言修复所保证的**不变量**：失败后目标与暂存都不存在 —— 于是 `dst.exists()`
    // 成为「搬运完整完成」的可信信号，下次重试必然重新搬。
    let dir = unique_dir("move-partial");
    write_file(&dir, "src/a.txt", "payload");
    let blocker = dir.join("blocker");
    write_file(&dir, "blocker", "i am a file");
    let dst = blocker.join("sub");

    assert!(move_entry(&dir.join("src"), &dst).is_err(), "复制必须失败");
    assert!(!dst.exists(), "失败后不得留下目标（否则会被当成「已完成」）");
    assert!(!staging_path(&dst).exists(), "失败后不得留下暂存残片");
    assert!(dir.join("src/a.txt").exists(), "源必须完好");

    // 换到可用目标重试 → 必须真的搬过去（证明「不会自解除」）
    let retry_dst = dir.join("retry");
    move_entry(&dir.join("src"), &retry_dst).expect("重试应成功");
    assert_eq!(
        std::fs::read_to_string(retry_dst.join("a.txt")).unwrap(),
        "payload"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn migrate_retries_entry_that_failed_before_instead_of_skipping() {
    // 承接上一条：失败项在下次 `migrate` 中必须被**重试**而不是跳过。
    let from = unique_dir("retry-entry-from");
    let to = unique_dir("retry-entry-to");
    write_file(&from, "config.json", "{}");

    // 制造失败：目标父路径是文件
    let blocker = to.join("blocker");
    write_file(&to, "blocker", "file");
    let blocked = blocker.join("sub");
    assert!(move_entry(&from.join("config.json"), &blocked).is_err());

    // 修好目标后重试：应搬成功，且**不是** skipped
    let outcome = migrate(&from, &to);
    assert!(outcome.is_ok(), "{:?}", outcome);
    assert!(
        outcome.moved.contains(&"config.json".to_string()),
        "上次失败的条目必须重试（moved），而不是被跳过: {:?}",
        outcome
    );
    assert!(to.join("config.json").exists());

    let _ = std::fs::remove_dir_all(&from);
    let _ = std::fs::remove_dir_all(&to);
}

#[test]
fn move_entry_cleans_stale_staging_before_retry() {
    // 上次失败可能残留暂存；重试必须先清掉，否则旧残片会混进这次的结果
    let dir = unique_dir("move-stale-staging");
    write_file(&dir, "src/new.txt", "fresh");
    let dst = dir.join("dst");
    write_file(&dir, "dst.hinina-partial/old.txt", "stale");

    move_entry(&dir.join("src"), &dst).expect("正常搬运应成功");

    assert!(dst.join("new.txt").exists(), "新内容应就位");
    assert!(!dst.join("old.txt").exists(), "旧残片不得混入目标");
    assert!(!staging_path(&dst).exists(), "暂存应已改名，不留残片");

    let _ = std::fs::remove_dir_all(&dir);
}

// ── 启动准备（解析 + 迁移） ──

#[test]
fn prepare_startup_uses_default_dir_when_no_pointer() {
    let default_dir = unique_dir("startup-default");
    let legacy = unique_dir("startup-default-legacy");

    let (plan, outcome) = prepare_startup(&default_dir, &legacy);

    assert_eq!(plan.base_dir, default_dir);
    assert_eq!(plan.default_dir, default_dir);
    assert_eq!(plan.source, DataDirSource::Default);
    assert!(outcome.is_noop(), "没有旧数据时不该有迁移动作");

    let _ = std::fs::remove_dir_all(&default_dir);
    let _ = std::fs::remove_dir_all(&legacy);
}

#[test]
fn prepare_startup_migrates_legacy_into_empty_default_dir() {
    let default_dir = unique_dir("startup-migrate");
    let legacy = unique_dir("startup-migrate-legacy");
    seed_legacy(&legacy);

    let (plan, outcome) = prepare_startup(&default_dir, &legacy);

    assert_eq!(plan.source, DataDirSource::Default);
    assert_eq!(plan.base_dir, default_dir);
    assert!(outcome.moved.contains(&"workspaces".to_string()), "{:?}", outcome);
    assert!(
        default_dir.join("workspaces/HOJ-1012-1000/main.cpp").exists(),
        "代码必须被搬到新目录（这是本次修复的核心目的）"
    );
    assert_eq!(
        read_pointer(&default_dir).legacy_migrated,
        Some(true),
        "成功后应置位一次性标记"
    );

    let _ = std::fs::remove_dir_all(&default_dir);
    let _ = std::fs::remove_dir_all(&legacy);
}

#[test]
fn prepare_startup_migrates_even_when_default_dir_has_webview_profile() {
    // **回归测试（实测踩到的缺陷）**：默认目录里几乎总是有 WebView2 的
    // `EBWebView/` profile —— 早先用「目标目录为空」当迁移门槛，导致对每个老用户
    // 都**永不迁移**（数据一直留在会被系统清理的临时目录里）。
    let default_dir = unique_dir("startup-webview");
    let legacy = unique_dir("startup-webview-legacy");
    seed_legacy(&legacy);
    // 模拟 WebView2 的 profile 目录（非空，但不是我们的数据）
    write_file(&default_dir, "EBWebView/Default/Cache/index", "binary-ish");

    let (plan, outcome) = prepare_startup(&default_dir, &legacy);

    assert_eq!(plan.source, DataDirSource::Default);
    assert!(outcome.is_ok(), "不应有失败项: {:?}", outcome);
    assert!(
        outcome.moved.contains(&"workspaces".to_string()),
        "即便默认目录非空（WebView profile），也必须完成迁移: {:?}",
        outcome
    );
    assert!(
        default_dir.join("workspaces/HOJ-1012-1000/main.cpp").exists(),
        "代码必须被搬到新目录"
    );
    // WebView 自己的文件不受影响
    assert!(default_dir.join("EBWebView/Default/Cache/index").exists());

    let _ = std::fs::remove_dir_all(&default_dir);
    let _ = std::fs::remove_dir_all(&legacy);
}

#[test]
fn prepare_startup_does_not_migrate_twice() {
    // 一次性标记的意义：用户在新目录里删掉的旧工作区**不该被搬回来**
    let default_dir = unique_dir("startup-once");
    let legacy = unique_dir("startup-once-legacy");
    seed_legacy(&legacy);

    let (_, first) = prepare_startup(&default_dir, &legacy);
    assert!(first.is_ok());
    assert_eq!(
        read_pointer(&default_dir).legacy_migrated,
        Some(true),
        "成功后应置位一次性标记"
    );

    // 用户在新目录里删掉了代码
    std::fs::remove_dir_all(default_dir.join("workspaces")).unwrap();

    let (_, second) = prepare_startup(&default_dir, &legacy);
    assert!(second.is_noop(), "第二次启动不该再迁移: {:?}", second);
    assert!(
        !default_dir.join("workspaces").exists(),
        "删掉的工作区不得被搬回来（一次性标记的核心作用）"
    );

    let _ = std::fs::remove_dir_all(&default_dir);
    let _ = std::fs::remove_dir_all(&legacy);
}

// 注：**「迁移失败时不置位标记」无法用单测确定性构造** —— 需要真实的 I/O 失败
// （文件被别的进程占用），而 `resolve` 已保证 base_dir 可用、`migrate` 又会跳过
// 目标已存在的条目（跳过不是失败）。故该决策由纯函数
// `pending_flag_is_kept_on_failure_and_cleared_on_success` 锁定，成功路径则由
// `prepare_startup_migrates_legacy_into_empty_default_dir` 断言标记已置位。

#[test]
fn prepare_startup_skips_legacy_entries_that_already_exist_at_target() {
    // 目标已有同名条目 → 跳过（不覆盖），且**不算失败**（故标记照常置位）
    let default_dir = unique_dir("startup-skip");
    let legacy = unique_dir("startup-skip-legacy");
    seed_legacy(&legacy);
    write_file(&default_dir, "config.json", r#"{"fresh":true}"#);

    let (_, outcome) = prepare_startup(&default_dir, &legacy);

    assert!(outcome.is_ok(), "跳过不是失败: {:?}", outcome);
    assert_eq!(outcome.skipped, vec!["config.json".to_string()]);
    assert_eq!(
        std::fs::read_to_string(default_dir.join("config.json")).unwrap(),
        r#"{"fresh":true}"#,
        "目标既有数据不得被覆盖"
    );
    // 其余条目照常搬过来（不是「目标非空就整体放弃」）
    assert!(default_dir.join("workspaces/HOJ-1012-1000/main.cpp").exists());
    assert_eq!(read_pointer(&default_dir).legacy_migrated, Some(true));

    let _ = std::fs::remove_dir_all(&default_dir);
    let _ = std::fs::remove_dir_all(&legacy);
}

#[test]
fn pending_flag_is_kept_on_failure_and_cleared_on_success() {
    // 决策锁定：「迁移失败时保留待迁移标记」无法用真实 I/O 失败确定性构造
    // （需要文件被别的进程占用），故把决策抽成纯函数单独锁定。
    let clean = MigrateOutcome {
        moved: vec!["config.json".into()],
        ..Default::default()
    };
    assert!(should_clear_pending(&clean), "全部成功 → 清除标记");

    let noop = MigrateOutcome::default();
    assert!(should_clear_pending(&noop), "无事可做也算成功 → 清除标记");

    let failed = MigrateOutcome {
        moved: vec!["config.json".into()],
        failed: vec!["workspaces".into()],
        ..Default::default()
    };
    assert!(
        !should_clear_pending(&failed),
        "有失败项 → 必须保留标记以便下次启动重试（否则数据永远留在旧目录）"
    );
}

#[test]
fn prepare_startup_does_not_migrate_legacy_when_custom_dir_is_set() {
    // 用户已指定别处：不该把临时目录的数据硬塞进他选的目录
    let default_dir = unique_dir("startup-custom-default");
    let custom = unique_dir("startup-custom-target");
    let legacy = unique_dir("startup-custom-legacy");
    seed_legacy(&legacy);
    std::fs::create_dir_all(&custom).unwrap();
    write_pointer(&default_dir, &pointer(Some(&custom), None)).expect("写指针失败");

    let (plan, outcome) = prepare_startup(&default_dir, &legacy);

    assert_eq!(plan.base_dir, custom);
    assert_eq!(plan.source, DataDirSource::Custom);
    assert!(outcome.is_noop(), "自定义目录 + 无待迁移标记 → 不迁移: {:?}", outcome);

    let _ = std::fs::remove_dir_all(&default_dir);
    let _ = std::fs::remove_dir_all(&custom);
    let _ = std::fs::remove_dir_all(&legacy);
}

#[test]
fn prepare_startup_honours_pending_migration_and_clears_flag() {
    // 设置页改目录的完整闭环：写指针（带 migrate_from）→ 下次启动搬运 → 清标记
    let default_dir = unique_dir("startup-pending-default");
    let source = unique_dir("startup-pending-source");
    let target = unique_dir("startup-pending-target");
    seed_legacy(&source);
    std::fs::create_dir_all(&target).unwrap();
    write_pointer(&default_dir, &pointer(Some(&target), Some(&source))).expect("写指针失败");

    let (plan, outcome) = prepare_startup(&default_dir, &source);

    assert_eq!(plan.base_dir, target);
    assert_eq!(plan.source, DataDirSource::Custom);
    assert!(outcome.moved.contains(&"workspaces".to_string()), "{:?}", outcome);
    assert!(target.join("workspaces/HOJ-1012-1000/main.cpp").exists());

    // 迁移完成 → 标记被清除（否则每次启动都重试）
    let pointer = read_pointer(&default_dir);
    assert_eq!(pointer.migrate_from, None, "迁移成功后应清除待迁移标记");
    assert_eq!(
        pointer.data_dir,
        Some(target.to_string_lossy().into_owned()),
        "用户指定的目录必须保留"
    );

    // 再次启动：无事可做
    let (_, second) = prepare_startup(&default_dir, &source);
    assert!(second.is_noop(), "第二次启动不该重复搬运: {:?}", second);

    let _ = std::fs::remove_dir_all(&default_dir);
    let _ = std::fs::remove_dir_all(&source);
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn prepare_startup_falls_back_and_still_migrates_when_custom_dir_unusable() {
    // 用户指定的目录不可用（这里是「指向一个文件」）时的完整行为：
    // resolve 优雅退回默认目录 → 迁移照常执行到默认目录 → 数据不丢。
    // 这是「宁可退到默认目录，也不能让数据搬不过去」的体现。
    let default_dir = unique_dir("startup-fallback-default");
    let source = unique_dir("startup-fallback-source");
    seed_legacy(&source);
    let holder = unique_dir("startup-fallback-holder");
    write_file(&holder, "blocker", "i am a file");
    let unusable = holder.join("blocker");
    write_pointer(&default_dir, &pointer(Some(&unusable), Some(&source))).expect("写指针失败");

    let (plan, outcome) = prepare_startup(&default_dir, &source);

    assert_eq!(plan.base_dir, default_dir, "指定目录不可用 → 退回默认目录");
    assert!(outcome.is_ok(), "迁移应照常完成: {:?}", outcome);
    assert!(
        default_dir.join("workspaces/HOJ-1012-1000/main.cpp").exists(),
        "数据必须落到可用的目录里（不能因为用户选错目录就搬不过去）"
    );
    assert_eq!(
        read_pointer(&default_dir).migrate_from,
        None,
        "迁移成功 → 清除待迁移标记"
    );

    let _ = std::fs::remove_dir_all(&default_dir);
    let _ = std::fs::remove_dir_all(&source);
    let _ = std::fs::remove_dir_all(&holder);
}

#[test]
fn resolve_falls_back_to_legacy_when_default_dir_unusable() {
    // 默认目录不可用（这里用一个「父路径是文件」的路径模拟不可创建）
    let blocker = unique_dir("resolve-blocked");
    write_file(&blocker, "afile", "x");
    let unusable_default = blocker.join("afile").join("sub");
    let legacy = unique_dir("resolve-blocked-legacy");

    let plan = resolve(&unusable_default, &legacy);

    assert_eq!(plan.source, DataDirSource::FallbackTemp, "不可用时必须回退");
    assert_eq!(plan.base_dir, legacy);
    assert_eq!(plan.default_dir, unusable_default, "仍要报告默认目录（供界面展示）");

    let _ = std::fs::remove_dir_all(&blocker);
    let _ = std::fs::remove_dir_all(&legacy);
}

#[test]
fn resolve_falls_back_to_default_when_custom_dir_unusable() {
    let default_dir = unique_dir("resolve-bad-custom-default");
    let legacy = unique_dir("resolve-bad-custom-legacy");
    let blocker = unique_dir("resolve-bad-custom");
    write_file(&blocker, "afile", "x");

    write_pointer(
        &default_dir,
        &pointer(Some(&blocker.join("afile").join("sub")), None),
    )
    .expect("写指针失败");

    let plan = resolve(&default_dir, &legacy);

    assert_eq!(plan.source, DataDirSource::Default, "指定目录不可用 → 退回默认");
    assert_eq!(plan.base_dir, default_dir);

    let _ = std::fs::remove_dir_all(&default_dir);
    let _ = std::fs::remove_dir_all(&legacy);
    let _ = std::fs::remove_dir_all(&blocker);
}
