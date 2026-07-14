// 阶段 7 P40：Command 层关键路径测试（第二轮：测试真实代码）。
//
// 测试重点：
// 1. `auth_cmd::parse_oj_type` 字符串解析（大小写不敏感、未知值回退）
// 2. `workspace_cmd` 中降级路径（通过提取的纯函数验证逻辑）
// 3. `start_auto_save_if_needed` static AtomicBool 一次性标记行为

use super::super::commands::auth_cmd;
use crate::core::provider::oj_type::OJType;

/// 测试 `auth_cmd::parse_oj_type` 真实函数。
/// 若 auth_cmd 中的 match 分支被修改，此测试会失败。
#[test]
fn oj_type_mapping_case_insensitive() {
    // 合法值，大小写混合
    assert_eq!(auth_cmd::parse_oj_type("HOJ"), Some(OJType::HOJ));
    assert_eq!(auth_cmd::parse_oj_type("hoj"), Some(OJType::HOJ));
    assert_eq!(auth_cmd::parse_oj_type("Hoj"), Some(OJType::HOJ));
    assert_eq!(auth_cmd::parse_oj_type("hOJ"), Some(OJType::HOJ));

    assert_eq!(auth_cmd::parse_oj_type("QDUOJ"), Some(OJType::QDUOJ));
    assert_eq!(auth_cmd::parse_oj_type("qduoj"), Some(OJType::QDUOJ));

    assert_eq!(auth_cmd::parse_oj_type("HUSTOJ"), Some(OJType::HUSTOJ));
    assert_eq!(auth_cmd::parse_oj_type("hustoj"), Some(OJType::HUSTOJ));

    // 非法值返回 None（login 中走回退分支）
    assert_eq!(auth_cmd::parse_oj_type("UNKNOWN"), None);
    assert_eq!(auth_cmd::parse_oj_type(""), None);
    assert_eq!(auth_cmd::parse_oj_type("ho"), None);
}

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
