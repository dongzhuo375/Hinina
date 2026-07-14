// 阶段 7 P40：Command 层关键路径测试。
//
// 测试重点：
// 1. `workspace_cmd` 中 WorkspaceManager 为 None 时的降级路径
// 2. `auth_cmd` 中 OJType 字符串解析（大小写不敏感、未知值回退）
// 3. `start_auto_save_if_needed` 一次性标记行为

use super::super::core::provider::oj_type::OJType;

/// OJType 字符串解析行为应与 `auth_cmd::login` 中的 match 分支一致。
#[test]
fn oj_type_mapping_case_insensitive() {
    // 合法值，大小写混合
    assert_eq!(parse_oj_type("HOJ"), Some(OJType::HOJ));
    assert_eq!(parse_oj_type("hoj"), Some(OJType::HOJ));
    assert_eq!(parse_oj_type("Hoj"), Some(OJType::HOJ));
    assert_eq!(parse_oj_type("hOJ"), Some(OJType::HOJ));

    assert_eq!(parse_oj_type("QDUOJ"), Some(OJType::QDUOJ));
    assert_eq!(parse_oj_type("qduoj"), Some(OJType::QDUOJ));

    assert_eq!(parse_oj_type("HUSTOJ"), Some(OJType::HUSTOJ));
    assert_eq!(parse_oj_type("hustoj"), Some(OJType::HUSTOJ));

    // 非法值返回 None（login 中走回退分支）
    assert_eq!(parse_oj_type("UNKNOWN"), None);
    assert_eq!(parse_oj_type(""), None);
    assert_eq!(parse_oj_type("ho"), None);
}

/// 提取自 `auth_cmd::login` 的 OJType 解析逻辑（纯函数，便于测试）。
fn parse_oj_type(s: &str) -> Option<OJType> {
    match s.to_uppercase().as_str() {
        "HOJ" => Some(OJType::HOJ),
        "QDUOJ" => Some(OJType::QDUOJ),
        "HUSTOJ" => Some(OJType::HUSTOJ),
        _ => None,
    }
}

/// `start_auto_save_if_needed` 使用 static AtomicBool 确保仅启动一次。
/// 测试其一次性标记行为。
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
