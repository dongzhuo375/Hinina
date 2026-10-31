use super::truncate_ellipsis;

#[test]
fn truncates_by_chars_and_appends_ellipsis_only_when_truncated() {
    assert_eq!(truncate_ellipsis("boom", 10), "boom");
    let long = "中".repeat(11);
    let out = truncate_ellipsis(&long, 10);
    assert_eq!(out.chars().count(), 11); // 10 + '…'
    assert!(out.ends_with('…'));
    // 恰好等于上限不截断
    let exact = "x".repeat(10);
    assert_eq!(truncate_ellipsis(&exact, 10), exact);
}
