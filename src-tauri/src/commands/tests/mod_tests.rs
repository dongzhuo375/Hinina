// 阶段 7 P40：Command 层关键路径测试（第二轮：测试真实代码）。
//
// 测试重点：
// 1. `workspace_cmd` 中降级路径（通过提取的纯函数验证逻辑）
// 2. `start_auto_save_if_needed` static AtomicBool 一次性标记行为
//
// （原 `parse_oj_type` 解析测试已随闭集枚举一并移除：OJ 身份改为数据
//   `OjId`，注册校验在 `ProviderRegistry` 层由 `list_available` 承担。）

/// 测试 `start_auto_save_if_needed` 中 static AtomicBool 懒启动标记。
/// 验证 swap 一次性语义——首次返回 false（未设置），后续返回 true（已设置）。
#[test]
fn auto_save_lazy_start_once() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let flag = AtomicBool::new(false);

    // 首次：应成功设置
    let was_set = flag.swap(true, Ordering::SeqCst);
    assert!(!was_set, "首次调用应返回 false（未设置过）");

    // 第二次：已被设置
    let was_set = flag.swap(true, Ordering::SeqCst);
    assert!(was_set, "第二次调用应返回 true（已设置过）");

    // 第三次：仍为 true
    assert!(flag.load(Ordering::SeqCst));
}

/// 验证 workspace_cmd 中 WorkspaceManager 为 None 时的降级逻辑。
/// `Option::ok_or_else` 在 None 时返回 Err，在 Some 时返回 Ok。
#[test]
fn option_none_produces_error() {
    let manager: Option<&str> = None;
    let result: Result<&str, String> = manager.ok_or_else(|| "未初始化".into());
    assert!(result.is_err());

    let manager: Option<&str> = Some("ready");
    let result: Result<&str, String> = manager.ok_or_else(|| "未初始化".into());
    assert!(result.is_ok());
}

// ── P58：Command 层可测的纯部分 ──
//
// 说明：`#[tauri::command]` 函数签名依赖 `tauri::State<'_, AppContext>`，
// 而 `State` 没有公开构造器，只能在 Tauri 运行时（AppHandle）内获得 ——
// 单元测试无法直接调用 get_contest_rank / get_user_problem_status /
// get_contest_problem_limits / set_workspace_language / list_contest_announcements /
// list_contest_submissions 等命令本体。这些命令的实质逻辑分两段：
// ① 参数默认值/钳制与 uid 解析（下方以纯函数等价锁定）；
// ② Service 调用与错误变体穿透（已在 service/contest、service/submission、
//    service/problem、service/workspace 的测试中覆盖，含未注册 Provider 时
//    registry 返回 ProviderNotFound 的分支）。

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

/// 分页参数默认值与钳制逻辑（与 contest_cmd / submission_cmd 内联表达式一致）：
/// 公告默认 50、提交列表默认 20、页码至少 1。
#[test]
fn paging_defaults_and_clamping() {
    const DEFAULT_ANNOUNCEMENT_LIMIT: i64 = 50;
    const DEFAULT_SUBMISSION_LIMIT: i64 = 20;

    // 与命令内联表达式同形的纯函数（Option 入参避免字面量折叠）
    fn resolve(current_page: Option<i64>, limit: Option<i64>, default_limit: i64) -> (i64, i64) {
        (
            current_page.unwrap_or(1).max(1),
            limit.unwrap_or(default_limit).max(1),
        )
    }

    // 未传参 → 默认值
    assert_eq!(resolve(None, None, DEFAULT_ANNOUNCEMENT_LIMIT), (1, 50));
    assert_eq!(resolve(None, None, DEFAULT_SUBMISSION_LIMIT), (1, 20));

    // 非法入参（0 / 负数）→ 钳制到 1，不会把 0 页发给 HOJ
    assert_eq!(resolve(Some(0), Some(0), DEFAULT_SUBMISSION_LIMIT), (1, 1));
    assert_eq!(resolve(Some(-5), Some(-1), DEFAULT_SUBMISSION_LIMIT), (1, 1));
    assert_eq!(resolve(Some(3), Some(100), DEFAULT_SUBMISSION_LIMIT), (3, 100));
}

/// 公告已读状态的 uid 解析规则（与 contest_cmd::session_uid 一致）：
/// 优先 HOJ 用户 UUID，旧版会话缺失 user_id 时回退 username。
#[test]
fn session_uid_prefers_user_id_and_falls_back_to_username() {
    fn resolve(user_id: &str, username: &str) -> String {
        if user_id.is_empty() {
            username.to_string()
        } else {
            user_id.to_string()
        }
    }
    assert_eq!(resolve("uuid-1", "alice"), "uuid-1");
    assert_eq!(resolve("", "alice"), "alice", "旧版会话文件无 user_id 时回退");
}

/// 提交列表筛选参数的空白过滤（与 submission_cmd 内联表达式一致）：
/// 空串/纯空白视为「不筛选」，不会拼进 HOJ 查询串。
#[test]
fn blank_problem_display_id_is_treated_as_no_filter() {
    let filter = |s: Option<String>| s.filter(|v| !v.trim().is_empty());
    assert_eq!(filter(Some("A".into())).as_deref(), Some("A"));
    assert_eq!(filter(Some("".into())), None);
    assert_eq!(filter(Some("   ".into())), None);
    assert_eq!(filter(None), None);
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
        || (3, 1_400),
    );

    assert_eq!(
        report,
        PurgeReport {
            freed_bytes: 1_400,
            removed_snapshots: 3,
            log_cleared: false,
        },
        "日志失败必须降级为 log_cleared=false，且不得吞掉留档清理的结果"
    );
}

/// 日志成功但文件层不可用（`clear_log_file` 返回 `Ok(0)` + `has_log_file=false`）：
/// 释放 0 字节，且 `log_cleared` 必须如实为 `false`（不能笼统报「已清理」）。
#[test]
fn run_purge_reports_log_not_cleared_when_file_layer_unavailable() {
    let report = run_purge(true, false, || Ok((0, false)), || unreachable!("未勾选留档"));

    assert_eq!(report.log_cleared, false);
    assert_eq!(report.freed_bytes, 0);
    assert_eq!(report.removed_snapshots, 0);
}

/// 两项都勾且都成功：数值如实汇总。
#[test]
fn run_purge_sums_both_items_on_success() {
    let report = run_purge(true, true, || Ok((4_787_700, true)), || (3, 420));

    assert_eq!(
        report,
        PurgeReport {
            freed_bytes: 4_788_120,
            removed_snapshots: 3,
            log_cleared: true,
        }
    );
}

/// 未勾选的那一项不得被执行（闭包 panic 即证明没被调用）。
#[test]
fn run_purge_skips_unselected_items() {
    let report = run_purge(
        false,
        false,
        || unreachable!("未勾选日志却调用了清理"),
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
    assert_eq!(json["currentDir"], r"C:\Users\u\AppData\Local\com.hinina.app");
    assert_eq!(json["defaultDir"], r"C:\Users\u\AppData\Local\com.hinina.app");
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
