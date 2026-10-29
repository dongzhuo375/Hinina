use super::requested;

#[test]
fn detects_exact_flag() {
    assert!(requested(&["--console".to_string()]));
}

#[test]
fn ignores_missing_near_miss_and_unrelated_args() {
    assert!(!requested(&[]));
    assert!(!requested(&["--consolex".to_string()]));
    assert!(!requested(&["-console".to_string()]));
    assert!(!requested(&["--console=1".to_string()]));
    assert!(!requested(&["--verbose".to_string(), "path".to_string()]));
}

#[test]
fn finds_flag_among_other_args() {
    assert!(requested(&[
        "--foo".to_string(),
        "--console".to_string(),
        "--bar".to_string()
    ]));
}

/// 非 Windows 平台 attach 是 no-op（跨平台编译一致；Windows 下 attach 涉及
/// 控制台 FFI 副作用，不进单测，由 release 人工验收覆盖）。
#[cfg(not(windows))]
#[test]
fn attach_is_noop_without_windows() {
    super::attach();
}
