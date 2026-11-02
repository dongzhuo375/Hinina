// 阶段 7 P40：Command 层关键路径测试（第二轮：测试真实代码）。
//
// 测试重点：
// 1. `workspace_cmd` 中降级路径（通过提取的纯函数验证逻辑）
// 2. auto-save 的「按配置同步」判据（P48 / P74 遗留项）
//
// （原 `parse_oj_type` 解析测试已随闭集枚举一并移除：OJ 身份改为数据
//   `OjId`，注册校验在 `ProviderRegistry` 层由 `list_available` 承担。
//   原 `auto_save_lazy_start_once` 锁定的是已删除的 static AtomicBool 标记 ——
//   那条判据本身就是缺陷，改由 `auto_save_action` 的判据表接管。）

/// auto-save「按配置同步」的判据表（P48 / P74 遗留项）。
///
/// 旧实现用一次性 `static AtomicBool` 懒启动：关掉 auto-save 后再打开永不重启，
/// 间隔也只读首次配置。判据现在是这张表 —— 它是「配置改动能否生效」的全部逻辑，
/// 因此逐格锁定。
#[test]
fn auto_save_action_decision_table() {
    use super::super::commands::workspace_cmd::auto_save_action;
    use super::super::commands::workspace_cmd::AutoSaveAction;

    // 未运行 + 配置开启 → 启动
    assert_eq!(auto_save_action(true, 30, None), AutoSaveAction::Start(30));
    // 未运行 + 配置关闭 → 保持（本就该停）
    assert_eq!(auto_save_action(false, 30, None), AutoSaveAction::Keep);
    // 未运行 + 间隔为 0 → 保持（间隔 0 等价于关闭，不得启动忙循环）
    assert_eq!(auto_save_action(true, 0, None), AutoSaveAction::Keep);

    // 运行中 + 间隔未变 → 保持（避免每次 load_workspace 都重置计时器）
    assert_eq!(auto_save_action(true, 30, Some(30)), AutoSaveAction::Keep);
    // 运行中 + 间隔变了 → 以新间隔重启
    assert_eq!(
        auto_save_action(true, 60, Some(30)),
        AutoSaveAction::Start(60)
    );
    // 运行中 + 配置关闭 → 停止（旧实现在这里再也起不来）
    assert_eq!(auto_save_action(false, 30, Some(30)), AutoSaveAction::Stop);
    // 运行中 + 间隔改为 0 → 停止
    assert_eq!(auto_save_action(true, 0, Some(30)), AutoSaveAction::Stop);
}

/// 「关掉再打开」必须能重新启动 —— 旧的一次性 static 标记正是在这里失败。
#[test]
fn auto_save_action_supports_off_then_on_cycle() {
    use super::super::commands::workspace_cmd::auto_save_action;
    use super::super::commands::workspace_cmd::AutoSaveAction;

    // 开启 → 关闭 → 再开启 的完整循环
    let mut running = None;
    assert_eq!(
        auto_save_action(true, 30, running),
        AutoSaveAction::Start(30)
    );
    running = Some(30);
    assert_eq!(auto_save_action(false, 30, running), AutoSaveAction::Stop);
    running = None;
    assert_eq!(
        auto_save_action(true, 30, running),
        AutoSaveAction::Start(30)
    );
}

// ── P58：Command 层可测的纯部分 ──
//
// M2 起，依赖 `State` 的命令已把编排提取为 `*_impl(&AppContext)` 并在各
// `tests/*_cmd_tests.rs` 用全真上下文直接测试（分页钳制、only_mine 强制、
// uid 解析等此前靠「纯函数等价锁定」的条目已由真实调用取代）。本文件保留
// 真正独立于命令编排的纯函数与 VO 锁定用例。

use super::super::commands::config_cmd::StorageInfo;

/// `get_storage_info` 返回值的线上 JSON 形状锁定（前端按 camelCase 键读取）。
#[test]
fn storage_info_serializes_camel_case() {
    let info = StorageInfo {
        base_dir: r"D:\data\hinina".into(),
        log_path: r"D:\data\hinina\logs\hinina.log".into(),
        version: "0.1.0".into(),
    };
    let json = serde_json::to_value(&info).expect("序列化失败");
    assert_eq!(json["baseDir"], r"D:\data\hinina");
    assert_eq!(json["logPath"], r"D:\data\hinina\logs\hinina.log");
    assert_eq!(json["version"], "0.1.0");
}

// ── 清理本地数据：逐项容错（两个勾选项互不牵连） ──
//
// `purge_local_data` 本体依赖 `State`，但其编排已抽成纯函数 `run_purge` ——
// 「日志清理失败不该连带取消用户已勾选的留档清理」正是这里最需要锁定的行为
// （曾经用 `?` 冒泡，日志一失败就整个命令报错返回，留档一条都没删）。

use super::super::commands::maintenance_cmd::{run_purge, PurgeReport};
use crate::core::error::AppError;

/// 日志清理失败时：报告里 `log_cleared=false`，但**留档清理照常执行**。
#[test]
fn run_purge_continues_snapshot_cleanup_when_log_clearing_fails() {
    let report = run_purge(
        true,
        true,
        || Err(AppError::Io("日志文件被占用".into())),
        || Ok(0),
        || (3, 1_400),
    );

    assert_eq!(
        report,
        PurgeReport {
            freed_bytes: 1_400,
            removed_snapshots: 3,
            log_cleared: false,
            crash_cleared: true,
        },
        "日志失败必须降级为 log_cleared=false，且不得吞掉留档清理的结果"
    );
}

/// 日志成功但文件层不可用（`clear_log_file` 返回 `Ok(0)` + `has_log_file=false`）：
/// 释放 0 字节，且 `log_cleared` 必须如实为 `false`（不能笼统报「已清理」）。
#[test]
fn run_purge_reports_log_not_cleared_when_file_layer_unavailable() {
    let report = run_purge(
        true,
        false,
        || Ok((0, false)),
        || Ok(0),
        || unreachable!("未勾选留档"),
    );

    assert!(!report.log_cleared);
    assert!(report.crash_cleared, "崩溃记录不存在也视为已清理");
    assert_eq!(report.freed_bytes, 0);
    assert_eq!(report.removed_snapshots, 0);
}

/// 两项都勾且都成功：数值如实汇总。
#[test]
fn run_purge_sums_both_items_on_success() {
    let report = run_purge(
        true,
        true,
        || Ok((4_787_700, true)),
        || Ok(512),
        || (3, 420),
    );

    assert_eq!(
        report,
        PurgeReport {
            freed_bytes: 4_788_632,
            removed_snapshots: 3,
            log_cleared: true,
            crash_cleared: true,
        }
    );
}

/// 回归：日志已截断、崩溃文件删除失败 —— 日志的字节数与 cleared 必须保留，
/// 崩溃记录单独标记失败（曾把两者包进一个 match，失败时整项作废、显示 0 B）。
#[test]
fn run_purge_keeps_log_result_when_crash_removal_fails() {
    let report = run_purge(
        true,
        false,
        || Ok((4_096, true)),
        || Err(AppError::Io("删除崩溃记录文件失败: 拒绝访问".into())),
        || unreachable!("未勾选留档"),
    );

    assert_eq!(report.freed_bytes, 4_096, "日志已释放的字节数不得被吞掉");
    assert!(report.log_cleared, "日志确实已清空");
    assert!(!report.crash_cleared, "崩溃记录删除失败必须单独回报");
}

/// 回归：日志失败、崩溃文件删除成功 —— 崩溃记录的字节数照常累计。
#[test]
fn run_purge_keeps_crash_result_when_log_clearing_fails() {
    let report = run_purge(
        true,
        false,
        || Err(AppError::Io("日志文件被占用".into())),
        || Ok(512),
        || unreachable!("未勾选留档"),
    );

    assert_eq!(report.freed_bytes, 512, "崩溃记录已删除的字节数不得被吞掉");
    assert!(!report.log_cleared);
    assert!(report.crash_cleared);
}

/// 未勾选的那一项不得被执行（闭包 panic 即证明没被调用）。
#[test]
fn run_purge_skips_unselected_items() {
    let report = run_purge(
        false,
        false,
        || unreachable!("未勾选日志却调用了清理"),
        || unreachable!("未勾选日志却调用了崩溃记录清理"),
        || unreachable!("未勾选留档却调用了清理"),
    );

    assert_eq!(report, PurgeReport::default());
}

// ── 数据目录：VO 线上形状锁定 ──

use super::super::commands::data_dir_cmd::{DataDirChange, DataDirInfo};
use crate::infra::data_dir::DataDirSource;

/// `get_data_dir` 返回值的 JSON 形状（前端按 camelCase 键读取）。
#[test]
fn data_dir_info_serializes_camel_case() {
    let info = DataDirInfo {
        current_dir: r"C:\Users\u\AppData\Local\com.hinina.app".into(),
        default_dir: r"C:\Users\u\AppData\Local\com.hinina.app".into(),
        source: DataDirSource::Default,
        restart_required: false,
    };
    let json = serde_json::to_value(&info).expect("序列化失败");
    assert_eq!(
        json["currentDir"],
        r"C:\Users\u\AppData\Local\com.hinina.app"
    );
    assert_eq!(
        json["defaultDir"],
        r"C:\Users\u\AppData\Local\com.hinina.app"
    );
    assert_eq!(json["restartRequired"], false);
    // source 是枚举字符串：前端据此区分「默认 / 用户指定 / 回退临时目录」
    assert_eq!(json["source"], "default");
}

#[test]
fn data_dir_source_serializes_all_variants() {
    // 三种来源都要能被前端区分 —— `fallbackTemp` 是界面必须显眼告警的那一种
    let to_str = |s: DataDirSource| serde_json::to_value(s).unwrap();
    assert_eq!(to_str(DataDirSource::Default), "default");
    assert_eq!(to_str(DataDirSource::Custom), "custom");
    assert_eq!(to_str(DataDirSource::FallbackTemp), "fallbackTemp");
}

#[test]
fn data_dir_change_serializes_camel_case_and_nullable_migrate_from() {
    let change = DataDirChange {
        target_dir: r"D:\hinina-data".into(),
        migrate_from: None,
        restart_required: true,
    };
    let json = serde_json::to_value(&change).expect("序列化失败");
    assert_eq!(json["targetDir"], r"D:\hinina-data");
    assert_eq!(json["restartRequired"], true);
    // 不迁移时为 null（前端据此不显示「将从 X 迁移」）
    assert!(json["migrateFrom"].is_null());
}

// ── 恢复默认数据目录：校验决策（HIGH 缺陷曾漏网的地方） ──
//
// `reset_data_dir` 本体依赖 `State` 无法单测，故校验抽成 `validate_reset` 纯函数 ——
// 判据用错（`dir_is_empty` 而非「冲突条目」）时，命令会在真机上永远失败。

use super::super::commands::contest_cmd::DEFAULT_RANK_LIMIT;
use super::super::commands::data_dir_cmd::validate_reset;
use crate::core::entity::rank::RankQuery;
use crate::test_support::TempDir;

/// 造独立临时目录（每个用例一个）。
///
/// 返回守卫：目录随 `Drop` 回收（此前只在开始时清理，长期跑测试会堆积残留）。
fn temp_dir(tag: &str) -> TempDir {
    TempDir::unique(&format!("hinina-test-reset-{tag}"))
}

fn write_at(dir: &std::path::Path, rel: &str, content: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

#[test]
fn validate_reset_allows_realistic_default_dir() {
    // **HIGH 缺陷回归**：真机上的默认目录必然含 `data_dir.json`（指针固定存于此）
    // 与 `EBWebView/`（每次启动重建），还可能有 `logs/`。用 `dir_is_empty` 当判据会
    // 让「恢复默认 + 迁移」**永远失败**，且指引是死循环（EBWebView 运行中被锁、
    // 删指针则按钮消失）。
    let default_dir = temp_dir("allow-default");
    let current = temp_dir("allow-current");
    write_at(
        default_dir.path(),
        "data_dir.json",
        r#"{"dataDir":"D:\\custom"}"#,
    );
    write_at(
        default_dir.path(),
        "EBWebView/Default/Cache/index",
        "binary",
    );
    write_at(default_dir.path(), "logs/hinina.log", "old log");

    assert!(
        validate_reset(default_dir.path(), current.path(), true).is_ok(),
        "这些条目都不在迁移清单里，不得阻止「恢复默认 + 迁移」"
    );
}

#[test]
fn validate_reset_rejects_stale_migratable_entries_when_migrating() {
    // 真正该拒的情形：默认目录残留会被迁移**跳过**的陈旧数据
    let default_dir = temp_dir("reject-stale");
    let current = temp_dir("reject-stale-current");
    write_at(default_dir.path(), "data_dir.json", "{}");
    write_at(default_dir.path(), "EBWebView/x", "binary");
    write_at(default_dir.path(), "config.json", r#"{"stale":true}"#);

    let err = validate_reset(default_dir.path(), current.path(), true)
        .expect_err("有陈旧 config.json 应拒绝");
    let msg = err.to_string();
    assert!(msg.contains("config.json"), "错误应点名冲突条目: {}", msg);
    assert!(
        !msg.contains("EBWebView") && !msg.contains("data_dir.json"),
        "不得把必然存在的条目也算成冲突（否则指引是死循环）: {}",
        msg
    );
}

#[test]
fn validate_reset_allows_stale_cache_only() {
    // `cache` 在迁移清单里但**非冲突条目**：缓存可重建，被跳过只是让新目录从空缓存
    // 开始（TTL 自然填充），不构成「静默使用陈旧数据」
    let default_dir = temp_dir("allow-cache");
    let current = temp_dir("allow-cache-current");
    write_at(default_dir.path(), "cache/problem_statement/1/A.json", "{}");

    assert!(
        validate_reset(default_dir.path(), current.path(), true).is_ok(),
        "残留旧 cache 不该阻止恢复默认"
    );
}

#[test]
fn validate_reset_skips_conflict_check_when_not_migrating() {
    // 不勾迁移时用户只是想切回默认目录，里面有什么就是什么
    let default_dir = temp_dir("no-migrate");
    let current = temp_dir("no-migrate-current");
    write_at(default_dir.path(), "config.json", r#"{"stale":true}"#);

    assert!(
        validate_reset(default_dir.path(), current.path(), false).is_ok(),
        "不迁移时无需校验冲突"
    );
}

#[test]
fn validate_reset_rejects_same_dir() {
    let dir = temp_dir("same");
    std::fs::create_dir_all(dir.path()).unwrap();

    let err = validate_reset(dir.path(), dir.path(), false).expect_err("已在默认目录应拒绝");
    assert!(err.to_string().contains("已在默认数据目录"));
}

/// 榜单默认分页大小「三处同值」的锁定（P71）。
///
/// 前端 `rank.service.ts` 的 `DEFAULT_RANK_PAGE_SIZE` 是唯一取值点；后端的
/// `DEFAULT_RANK_LIMIT` 与 `RankQuery::default()` 只服务于「绕过前端直接调命令」。
/// 这类漂移**没有任何运行时症状**（前端恒显式传参），只能靠用例锁住。
#[test]
fn rank_default_page_size_matches_frontend_contract() {
    assert_eq!(
        DEFAULT_RANK_LIMIT, 50,
        "须与 src/services/rank.service.ts 的 DEFAULT_RANK_PAGE_SIZE 同值"
    );
    assert_eq!(
        RankQuery::default().limit,
        DEFAULT_RANK_LIMIT,
        "RankQuery::default() 须与命令层默认值同值"
    );
    assert_eq!(RankQuery::default().current_page, 1, "默认第 1 页");
}

// ── update_config 的「维护者层锁定」语义（评审二十六 #2） ──

#[tokio::test]
async fn locked_update_preserves_invalid_locked_fields_and_saves_editable_ones() {
    use super::super::commands::config_cmd::apply_config_update;
    use crate::test_support::app_ctx::context_with_hoj;

    let (ctx, _dir) = context_with_hoj("https://hoj.example").await;
    // 手改出的非法锁定值：地址非法 + 超时越界
    ctx.config
        .update(|cfg| {
            cfg.oj.instances[0].base_url = "not-a-url".into();
            cfg.oj.timeout_secs = 999;
        })
        .expect("注入非法配置");

    // 选手层修改字号；锁定字段按「旧值」提交（前端会原样带回）
    let mut payload = ctx.config.get();
    payload.editor.font_size = 20;
    payload.oj.timeout_secs = 999;
    payload.oj.instances[0].base_url = "tampered".into(); // 锁定层的新值必须被旧值覆盖

    apply_config_update(&ctx, payload, true).expect("锁定模式不得因非法锁定值拒绝保存");

    let saved = ctx.config.get();
    assert_eq!(saved.editor.font_size, 20, "可编辑字段应保存");
    assert_eq!(saved.oj.timeout_secs, 999, "越界锁定值不得被钳制");
    assert_eq!(
        saved.oj.instances[0].base_url, "not-a-url",
        "锁定地址必须保持旧值（不 trim、不改写）"
    );
}

#[tokio::test]
async fn unlocked_update_still_rejects_invalid_url_and_clamps() {
    use super::super::commands::config_cmd::apply_config_update;
    use crate::test_support::app_ctx::context_with_hoj;

    let (ctx, _dir) = context_with_hoj("https://hoj.example").await;
    let mut payload = ctx.config.get();
    payload.oj.instances[0].base_url = "not-a-url".into();
    let err = apply_config_update(&ctx, payload, false).expect_err("普通路径仍拒绝非法地址");
    assert!(err.to_string().contains("http"), "应提示地址非法: {err}");
}

/// 锁定模式刻意保留的结构性检查：active 指向缺失实例时仍拒绝保存
///（手改配置的该场景由 `--settings` 修复，评审二十七观察项）。
#[tokio::test]
async fn locked_update_still_rejects_active_pointing_at_missing_instance() {
    use super::super::commands::config_cmd::apply_config_update;
    use crate::test_support::app_ctx::context_with_hoj;

    let (ctx, _dir) = context_with_hoj("https://hoj.example").await;
    ctx.config
        .update(|cfg| {
            cfg.oj.active = "Ghost".into();
        })
        .expect("注入缺失实例");

    let payload = ctx.config.get();
    let err = apply_config_update(&ctx, payload, true).expect_err("结构性检查仍应拒绝");
    assert!(err.to_string().contains("不在已启用的实例列表"), "{err}");
}

/// 普通保存不得改写 `oj.active`（评审二十九 #1）：active 只由 switch_oj 改写，
/// 旧快照与在途切换交错时不能把新 active 覆盖回去。
#[tokio::test]
async fn update_config_never_overwrites_active() {
    use super::super::commands::config_cmd::apply_config_update;
    use crate::test_support::app_ctx::context_with_hoj;

    let (ctx, _dir) = context_with_hoj("https://hoj.example").await;
    let mut payload = ctx.config.get();
    payload.oj.active = "Ghost".into(); // 陈旧/伪造快照
    payload.editor.font_size = 20;

    apply_config_update(&ctx, payload, false).expect("普通保存应成功");

    let saved = ctx.config.get();
    assert_eq!(saved.oj.active, "HOJ", "active 必须保持内存当前值");
    assert_eq!(saved.editor.font_size, 20);
}

/// 评审三十三 #2：保存与切换交错不得落盘一份 `AppConfig::validate()` 拒绝的配置。
///
/// 载荷删除 Hydro、active 仍是保存开始时的陈旧值 Hydro。保存进入写事务后
/// （`before_validate` 注入点）另一线程发起切到 Hydro：
/// - 旧实现先按旧 active 校验、再在锁内把 active 换成最新值，交错下会保存
///   「active=Hydro 但实例已删除」的非法配置；
/// - 现在校验发生在写锁内、针对固定 active 后的**最终候选**，且切换被同一把
///   事务锁挡到保存之后——切换会因「Hydro 不在启用实例列表」而失败。
#[tokio::test]
async fn concurrent_switch_cannot_make_saved_config_invalid() {
    use super::super::commands::config_cmd::apply_config_update_with_hook;
    use crate::core::entity::config::AppConfig;
    use crate::test_support::app_ctx::context_with_two_ojs;
    use crate::test_support::loopback::LoopbackServer;

    let hoj = LoopbackServer::start(vec![]).await;
    let hydro = LoopbackServer::start(vec![]).await;
    let (ctx, dir) = context_with_two_ojs(hoj.url(), hydro.url()).await;

    let mut payload = ctx.config.get();
    payload
        .oj
        .instances
        .retain(|instance| instance.id != "Hydro");
    payload.oj.active = "Hydro".into(); // 陈旧快照：真正的 active 仍是 HOJ
    payload.editor.font_size = 18;

    let (start_tx, start_rx) = std::sync::mpsc::channel::<()>();
    let (held_tx, held_rx) = std::sync::mpsc::channel::<bool>();
    let mut before_validate = || {
        // **时序握手**（评审三十四/三十五）：保存此刻已持有写事务，再放切换
        // 线程进来。切换在**事务入口、取锁之前**用 try_lock 回报「锁已被保存
        // 持有」，保证它只能在保存提交后继续读到最终候选——不再靠 spawn 抢跑
        // 或 sleep 猜顺序。该回报是「进入竞争窗口」的证据，不证明切换自身随后
        // 一定拿到锁（那由 switch/switch 用例单独覆盖）。
        start_tx.send(()).unwrap();
        let held = held_rx.recv().unwrap();
        assert!(held, "切换进入事务时，写事务必须已被保存持有");
    };

    let (saved, switched) = std::thread::scope(|scope| {
        let ctx_ref = &ctx;
        let enqueued = scope.spawn(move || {
            start_rx.recv().unwrap();
            crate::commands::oj_cmd::switch_oj_with_hooks(
                ctx_ref,
                "Hydro",
                |ctx_probe| {
                    held_tx.send(ctx_probe.config.write_txn_is_held()).unwrap();
                },
                || {},
                || {},
            )
        });
        let saved = apply_config_update_with_hook(&ctx, payload, false, &mut before_validate);
        (saved, enqueued.join().unwrap())
    });

    saved.expect("保存应成功：最终 active 指向仍启用的 HOJ");
    assert!(
        switched.is_err(),
        "目标已被本次保存删除，切换必须失败，实得 {:?}",
        switched
    );

    let memory = ctx.config.get();
    memory
        .validate()
        .expect("内存配置必须通过 AppConfig::validate");
    let disk: AppConfig =
        serde_json::from_str(&std::fs::read_to_string(dir.join("config.json")).unwrap()).unwrap();
    disk.validate()
        .expect("磁盘配置必须通过 AppConfig::validate");
    assert_eq!(disk.oj.active, memory.oj.active);
    assert_eq!(memory.oj.active, "HOJ");
    assert_eq!(memory.editor.font_size, 18, "可编辑字段仍应保存");

    hoj.shutdown();
    hydro.shutdown();
}

/// 评审三十四残余：`reload_config` 必须把 Registry 同步到磁盘 `oj.active`。
///
/// 旧实现只替换内存配置：磁盘写着 Hydro、内存也读成 Hydro，但 Registry 仍是
/// HOJ——之后所有查询继续打旧 OJ，配置却声称已切新 OJ。现在重载与 Registry
/// 切换在同一配置写事务内完成。
#[tokio::test]
async fn reload_config_synchronises_registry_with_disk_active() {
    use super::super::commands::config_cmd::reload_config_impl;
    use crate::test_support::app_ctx::context_with_two_ojs;
    use crate::test_support::loopback::LoopbackServer;

    let hoj = LoopbackServer::start(vec![]).await;
    let hydro = LoopbackServer::start(vec![]).await;
    let (ctx, dir) = context_with_two_ojs(hoj.url(), hydro.url()).await;
    assert_eq!(ctx.provider_registry.current_id().as_str(), "HOJ");

    // 运行中直接改盘：active 指向 Hydro（模拟手改 config.json / 外部写入）
    let mut disk_config = ctx.config.get();
    disk_config.oj.active = "Hydro".into();
    std::fs::write(
        dir.join("config.json"),
        serde_json::to_string(&disk_config).unwrap(),
    )
    .unwrap();

    let reloaded = reload_config_impl(&ctx).expect("重载应成功");
    assert_eq!(reloaded.oj.active, "Hydro");
    assert_eq!(ctx.config.get().oj.active, "Hydro", "内存跟随磁盘");
    assert_eq!(
        ctx.provider_registry.current_id().as_str(),
        "Hydro",
        "Registry 必须同步到磁盘 active，不得留分叉"
    );

    hoj.shutdown();
    hydro.shutdown();
}

/// 评审三十五 #2：reload 的目标预检失败时，不得先提交内存/事件。
///
/// 磁盘包含已启用但没有适配器的 Ghost 且 active=Ghost：归一化会保留它（实例确实
/// enabled）。旧实现先 `reload_locked` 提交内存并发布 ConfigChanged，再发现
/// Ghost 不可用而返回错误——留下内存/磁盘=Ghost、Registry=旧 OJ 的分叉。
/// 现在预检在提交前完成：内存、Registry、事件全部保持原样。
#[tokio::test]
async fn reload_config_rejects_unknown_active_without_committing() {
    use super::super::commands::config_cmd::reload_config_impl;
    use crate::core::entity::config::OjInstance;
    use crate::test_support::app_ctx::context_with_two_ojs;
    use crate::test_support::loopback::LoopbackServer;

    let hoj = LoopbackServer::start(vec![]).await;
    let hydro = LoopbackServer::start(vec![]).await;
    let (ctx, dir) = context_with_two_ojs(hoj.url(), hydro.url()).await;
    let before = ctx.config.get();
    let mut events = ctx.event_bus.subscribe();

    let mut disk_config = ctx.config.get();
    disk_config.oj.instances.push(OjInstance {
        id: "Ghost".into(),
        base_url: "https://ghost.example".into(),
        enabled: true,
        options: serde_json::Map::new(),
    });
    disk_config.oj.active = "Ghost".into();
    std::fs::write(
        dir.join("config.json"),
        serde_json::to_string(&disk_config).unwrap(),
    )
    .unwrap();

    let err = reload_config_impl(&ctx).expect_err("未知 active 必须拒绝重载");
    assert!(
        matches!(err, crate::core::error::AppError::ProviderNotFound(_)),
        "应为 ProviderNotFound，实得 {:?}",
        err
    );

    assert_eq!(ctx.config.get(), before, "内存配置不得被提交");
    assert_eq!(
        ctx.provider_registry.current_id().as_str(),
        "HOJ",
        "Registry 不得改变"
    );
    assert!(
        events.try_recv().is_err(),
        "失败的重载不得发布 ConfigChanged / OjSwitched"
    );

    hoj.shutdown();
    hydro.shutdown();
}

/// 评审三十六 P2：Registry 未变（HOJ）时也不能跳过 active 的结构检查。
///
/// 归一化在「零启用实例」时会回退默认 HOJ（即使 HOJ 被禁用），若只在
/// registry_changed 时预检，会提交一份 `AppConfig::validate()` 拒绝的配置。
#[tokio::test]
async fn reload_config_rejects_all_disabled_instances_even_if_registry_unchanged() {
    use super::super::commands::config_cmd::reload_config_impl;
    use crate::test_support::app_ctx::context_with_two_ojs;
    use crate::test_support::loopback::LoopbackServer;

    let hoj = LoopbackServer::start(vec![]).await;
    let hydro = LoopbackServer::start(vec![]).await;
    let (ctx, dir) = context_with_two_ojs(hoj.url(), hydro.url()).await;
    let before = ctx.config.get();
    let mut events = ctx.event_bus.subscribe();

    let mut disk_config = ctx.config.get();
    for instance in &mut disk_config.oj.instances {
        instance.enabled = false;
    }
    disk_config.oj.active = "HOJ".into(); // 与 Registry 相同，registry_changed=false
    std::fs::write(
        dir.join("config.json"),
        serde_json::to_string(&disk_config).unwrap(),
    )
    .unwrap();

    let err = reload_config_impl(&ctx).expect_err("全禁用配置必须拒绝重载");
    assert!(
        matches!(err, crate::core::error::AppError::Config(_)),
        "应为 Config，实得 {:?}",
        err
    );

    let memory = ctx.config.get();
    memory.validate().expect("内存配置必须保持有效");
    assert_eq!(memory, before, "内存不得被提交");
    assert_eq!(ctx.provider_registry.current_id().as_str(), "HOJ");
    assert!(events.try_recv().is_err(), "失败不得发布任何配置事件");

    hoj.shutdown();
    hydro.shutdown();
}

/// 评审三十六 P2：`commit + auto-save 同步` 必须在同一写事务内。
///
/// U1 保存 autoSave=false，同步读取旧值后在 hook 暂停；U2 保存 autoSave=true
/// 并启动任务。若同步在锁外，U2 先完成后 U1 会用旧值 stop，最终磁盘开启而
/// 管理器停止。锁内同步使 U1 持锁完成 stop 后 U2 才提交，最终任务按 U2 启动。
#[tokio::test]
async fn auto_save_sync_is_serialised_with_config_commit() {
    use super::super::commands::config_cmd::{
        apply_config_update, apply_config_update_with_sync_hook,
    };
    use crate::test_support::app_ctx::context_with_hoj;

    let (ctx, _dir) = context_with_hoj("https://hoj.example").await;
    let mut off = ctx.config.get();
    off.editor.auto_save = false;
    let mut on = ctx.config.get();
    on.editor.auto_save = true;
    on.editor.auto_save_interval_secs = 7;

    let (start_tx, start_rx) = std::sync::mpsc::channel::<()>();
    let (held_tx, held_rx) = std::sync::mpsc::channel::<bool>();
    let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
    let mut after_sync_read = || {
        // 同步已读取配置（autoSave=false），此刻若在锁外，U2 可以抢跑
        start_tx.send(()).unwrap();
        let held = held_rx.recv().unwrap();
        if !held {
            done_rx.recv().expect("无锁路径下 U2 应先完成");
        }
    };

    let (r1, r2) = std::thread::scope(|scope| {
        let ctx_ref = &ctx;
        let handle = tokio::runtime::Handle::current();
        let u2 = scope.spawn(move || {
            start_rx.recv().unwrap();
            // start_auto_save 内部 tokio::spawn 需要 runtime 上下文
            let _enter = handle.enter();
            let held = ctx_ref.config.write_txn_is_held();
            held_tx.send(held).unwrap();
            let result = apply_config_update(ctx_ref, on, false);
            if !held {
                let _ = done_tx.send(());
            }
            result
        });
        let r1 = apply_config_update_with_sync_hook(&ctx, off, false, &mut after_sync_read);
        (r1, u2.join().unwrap())
    });

    r1.expect("U1 保存成功");
    r2.expect("U2 保存成功");
    let wm = ctx
        .workspace_manager
        .as_ref()
        .expect("测试上下文应有工作区管理器");
    assert_eq!(
        wm.auto_save_interval_secs(),
        Some(7),
        "最终 auto-save 必须按最后落盘的配置运行"
    );
    let saved = ctx.config.get();
    assert!(saved.editor.auto_save);
    assert_eq!(saved.editor.auto_save_interval_secs, 7);
}

/// 评审三十六 P2：reload 的配置提交与 Registry 切换必须在同一访问栅栏内。
#[tokio::test]
async fn reload_holds_access_fence_across_commit_and_registry_switch() {
    use super::super::commands::config_cmd::reload_config_impl_with_hook;
    use crate::test_support::app_ctx::context_with_two_ojs;
    use crate::test_support::loopback::LoopbackServer;

    let hoj = LoopbackServer::start(vec![]).await;
    let hydro = LoopbackServer::start(vec![]).await;
    let (ctx, dir) = context_with_two_ojs(hoj.url(), hydro.url()).await;
    let mut disk_config = ctx.config.get();
    disk_config.oj.active = "Hydro".into();
    std::fs::write(
        dir.join("config.json"),
        serde_json::to_string(&disk_config).unwrap(),
    )
    .unwrap();

    let mut fenced_between_commit_and_switch = false;
    let reloaded = reload_config_impl_with_hook(&ctx, || {
        fenced_between_commit_and_switch = ctx.access.snapshot().is_none();
    })
    .expect("重载应成功");
    assert_eq!(reloaded.oj.active, "Hydro");
    assert!(
        fenced_between_commit_and_switch,
        "提交与 Registry 切换之间必须保持访问栅栏"
    );
    assert!(ctx.access.snapshot().is_some(), "重载结束后必须释放栅栏");

    hoj.shutdown();
    hydro.shutdown();
}

/// 评审三十八 P2：`load_workspace` 的「读配置 + 同步 auto-save」也必须与
/// 配置保存共用同一串行域，否则仍可复现「load 按旧快照 stop 覆盖新 save」。
#[tokio::test]
async fn workspace_load_sync_is_serialised_with_config_save() {
    use super::super::commands::config_cmd::apply_config_update;
    use super::super::commands::workspace_cmd::load_workspace_sync_auto_save_with_hook;
    use crate::test_support::app_ctx::context_with_hoj;

    let (ctx, _dir) = context_with_hoj("https://hoj.example").await;
    // 初始关闭 auto-save，让 load 读到旧值 false
    ctx.config
        .update(|cfg| cfg.editor.auto_save = false)
        .expect("注入关闭状态");
    let mut on = ctx.config.get();
    on.editor.auto_save = true;
    on.editor.auto_save_interval_secs = 7;

    let (start_tx, start_rx) = std::sync::mpsc::channel::<()>();
    let (held_tx, held_rx) = std::sync::mpsc::channel::<bool>();
    let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
    let after_read = || {
        start_tx.send(()).unwrap();
        let held = held_rx.recv().unwrap();
        if !held {
            done_rx.recv().expect("无锁路径下 U2 应先完成");
        }
    };

    std::thread::scope(|scope| {
        let ctx_ref = &ctx;
        let handle = tokio::runtime::Handle::current();
        let u2 = scope.spawn(move || {
            start_rx.recv().unwrap();
            let _enter = handle.enter();
            let held = ctx_ref.config.write_txn_is_held();
            held_tx.send(held).unwrap();
            let result = apply_config_update(ctx_ref, on, false);
            if !held {
                let _ = done_tx.send(());
            }
            result
        });
        load_workspace_sync_auto_save_with_hook(&ctx, after_read);
        u2.join().unwrap().expect("U2 保存成功");
    });

    let wm = ctx
        .workspace_manager
        .as_ref()
        .expect("测试上下文应有工作区管理器");
    assert_eq!(
        wm.auto_save_interval_secs(),
        Some(7),
        "load_workspace 不得用旧快照覆盖并发保存后的启停"
    );
    assert!(ctx.config.get().editor.auto_save);
}
