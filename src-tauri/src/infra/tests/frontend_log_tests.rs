use super::{
    budget_snapshot_for_tests, consume_budget, sanitize_level, sanitize_message, sanitize_scope,
    set_budget_for_tests, BudgetDecision, BUDGET_WINDOW_BYTES, BUDGET_WINDOW_SECS,
};

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
    // 换行与制表保留：栈帧可读（前端自 PR #82 起上报 Error.stack）
    assert_eq!(sanitize_message("line1\nline2\tend"), "line1\nline2\tend");
}

/// 滚动窗口预算：记账、丢弃计数与首丢弃标记、窗口滚动恢复。
/// 预算静态量是进程级单例 —— 全部预算断言收进一个用例串行执行，
/// 不与并行测试争状态（sanitize 用例不触碰预算）。
#[test]
fn budget_window_accounts_resets_and_counts_drops() {
    // 新窗口：放行并按消息字节数记账
    set_budget_for_tests(0, 0, 0);
    assert!(matches!(consume_budget(10), BudgetDecision::Allow));
    assert_eq!(budget_snapshot_for_tests(), (10, 0));

    // 窗口已满（新窗口，seconds_ago=0 不滚动）：丢弃，首次丢弃带 first 标记
    set_budget_for_tests(0, BUDGET_WINDOW_BYTES, 0);
    assert!(matches!(
        consume_budget(1),
        BudgetDecision::Drop { first: true }
    ));
    assert_eq!(budget_snapshot_for_tests().1, 1);
    assert!(matches!(
        consume_budget(1),
        BudgetDecision::Drop { first: false }
    ));
    assert_eq!(budget_snapshot_for_tests().1, 2);

    // 窗口滚动（起点已过期 ≥ 60s）：记账与丢弃计数清零，预算恢复
    set_budget_for_tests(BUDGET_WINDOW_SECS, BUDGET_WINDOW_BYTES, 5);
    assert!(matches!(consume_budget(10), BudgetDecision::Allow));
    assert_eq!(budget_snapshot_for_tests(), (10, 0));

    // 收尾恢复新窗口（进程级，测试运行器共享静态量）
    set_budget_for_tests(0, 0, 0);
}

/// 窗口边界：恰好差 1 秒不过期（不滚动、沿用记账），恰好到 60 秒过期。
#[test]
fn budget_window_expiry_boundary() {
    // 59 秒前开始的窗口：未过期，沿用记账
    set_budget_for_tests(BUDGET_WINDOW_SECS - 1, 100, 0);
    assert!(matches!(consume_budget(50), BudgetDecision::Allow));
    assert_eq!(budget_snapshot_for_tests(), (150, 0));

    // 60 秒前开始的窗口：过期，滚动后放行且清账
    set_budget_for_tests(BUDGET_WINDOW_SECS, 100, 3);
    assert!(matches!(consume_budget(50), BudgetDecision::Allow));
    assert_eq!(budget_snapshot_for_tests(), (50, 0));
}
