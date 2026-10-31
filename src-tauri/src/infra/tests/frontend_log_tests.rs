use super::{sanitize_level, sanitize_message, sanitize_scope, set_budget_for_tests};

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

/// 会话预算：预算内放行、耗尽后丢弃并在**首次丢弃**时告警一行。
/// 预算静态量是进程级单例 —— 全部预算断言收进一个用例串行执行，
/// 不与并行测试争状态（sanitize 用例不触碰预算）。
#[test]
fn session_budget_gates_emit() {
    // 预算归零：直接进入丢弃分支，第一条丢弃产生一条告警（经 tracing 落
    // stderr；测试构建无订阅器，这里只验证不 panic 且后续静默）
    set_budget_for_tests(0);
    for _ in 0..3 {
        super::emit("error", "vue", "budget probe");
    }

    // 小预算：小消息放行（扣减后清零）、下一条同大小被丢弃
    set_budget_for_tests(4);
    super::emit("error", "vue", "abcd"); // 恰好耗尽，放行
    super::emit("error", "vue", "abcd"); // 预算 0，丢弃
    super::emit("error", "vue", "x"); // 丢弃

    // 大消息（超过剩余预算）也放行直到预算触底：预算是近似闸门而非精确记账
    set_budget_for_tests(10);
    super::emit("error", "vue", "0123456789abcdef"); // 放行，预算清零
    super::emit("error", "vue", "x"); // 丢弃

    // 收尾恢复满预算（进程级，测试运行器共享静态量）
    set_budget_for_tests(64 * 1024);
}
