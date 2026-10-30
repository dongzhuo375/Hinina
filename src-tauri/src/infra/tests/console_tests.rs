use super::requested;
use std::ffi::OsString;

fn os(value: &str) -> OsString {
    OsString::from(value)
}

#[test]
fn detects_exact_flag() {
    assert!(requested(&[os("--console")]));
}

#[test]
fn ignores_missing_near_miss_and_unrelated_args() {
    assert!(!requested(&[]));
    assert!(!requested(&[os("--consolex")]));
    assert!(!requested(&[os("-console")]));
    assert!(!requested(&[os("--console=1")]));
    assert!(!requested(&[os("--verbose"), os("path")]));
}

#[test]
fn finds_flag_among_other_args() {
    assert!(requested(&[os("--foo"), os("--console"), os("--bar")]));
}

/// 非 Windows 平台 attach 是 no-op（跨平台编译一致；Windows 下 attach 涉及
/// 控制台 FFI 副作用，不进单测，由 release 人工验收覆盖）。
#[cfg(not(windows))]
#[test]
fn attach_is_noop_without_windows() {
    super::attach();
}

/// requested 与 OsStr 精确比较：混入非 Unicode 参数也不炸（对照
/// `std::env::args()` 在同一输入上会 panic），且仍能认出 `--console`。
#[test]
fn compares_as_os_str_not_utf8() {
    #[cfg(unix)]
    fn non_unicode() -> OsString {
        use std::os::unix::ffi::OsStringExt;
        OsString::from_vec(vec![0xff])
    }
    #[cfg(windows)]
    fn non_unicode() -> OsString {
        // 未配对代理项：合法 WTF-16，但不是有效 Unicode 标量序列
        use std::os::windows::ffi::OsStringExt;
        OsString::from_wide(&[0xD800])
    }
    #[cfg(not(any(unix, windows)))]
    fn non_unicode() -> OsString {
        OsString::from("?")
    }

    assert!(requested(&[non_unicode(), os("--console")]));
}
