use super::{sanitize_level, sanitize_message, sanitize_scope};

#[test]
fn level_whitelist_maps_unknown_to_warn() {
    assert_eq!(sanitize_level("error"), "error");
    assert_eq!(sanitize_level("warn"), "warn");
    assert_eq!(sanitize_level("info"), "info");
    assert_eq!(sanitize_level("debug"), "debug");
    // 未知/大小写不符/空串都归 warn：可见但不制造 error 噪音
    assert_eq!(sanitize_level("critical"), "warn");
    assert_eq!(sanitize_level("ERROR"), "warn");
    assert_eq!(sanitize_level(""), "warn");
}

#[test]
fn scope_allowlist_follows_safe_id_model() {
    assert_eq!(sanitize_scope("workspaceStore"), "workspaceStore");
    assert_eq!(sanitize_scope("ipc.bridge-x_1"), "ipc.bridge-x_1");
    // 非法字符 / 超长 / 空串 → 统一占位
    assert_eq!(sanitize_scope("has space"), "<invalid>");
    assert_eq!(sanitize_scope("中文"), "<invalid>");
    assert_eq!(sanitize_scope("newline\n"), "<invalid>");
    assert_eq!(sanitize_scope(""), "<invalid>");
    let long = "a".repeat(65);
    assert_eq!(sanitize_scope(&long), "<invalid>");
    // 恰好 64 字符合法
    let max = "a".repeat(64);
    assert_eq!(sanitize_scope(&max), max.as_str());
}

#[test]
fn message_truncates_by_chars_not_bytes() {
    assert_eq!(sanitize_message("boom"), "boom");
    // 中文按字符截断：2000 个字符（3000 字节）后加省略标记
    let long = "中".repeat(2001);
    let truncated = sanitize_message(&long);
    assert_eq!(truncated.chars().count(), 2001); // 2000 + '…'
    assert!(truncated.ends_with('…'));
    // 恰好 2000 字符不截断
    let exact = "x".repeat(2000);
    assert_eq!(sanitize_message(&exact), exact);
}

#[test]
fn message_strips_control_chars_but_keeps_stack_shape() {
    // \r 与其它 C0 控制字符替换为空格（防伪造行首 / 终端转义注入）
    assert_eq!(sanitize_message("a\rb\u{1}c"), "a b c");
    // 换行与制表保留：栈帧可读
    assert_eq!(sanitize_message("line1\nline2\tend"), "line1\nline2\tend");
}
