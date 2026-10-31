use super::*;
use std::ffi::OsString;

fn os(value: &str) -> OsString {
    OsString::from(value)
}

#[test]
fn detects_exact_flag() {
    assert!(requested(&[os("--settings")]));
}

#[test]
fn ignores_missing_near_miss_and_unrelated_args() {
    assert!(!requested(&[]));
    assert!(!requested(&[os("--settingsx")]));
    assert!(!requested(&[os("-settings")]));
    assert!(!requested(&[os("--settings=1")]));
    assert!(!requested(&[os("--console"), os("path")]));
}

#[test]
fn finds_flag_among_other_args() {
    assert!(requested(&[os("--console"), os("--settings"), os("--bar")]));
}

/// requested 与 OsStr 精确比较：混入非 Unicode 参数也不炸（对照
/// `std::env::args()` 在同一输入上会 panic），且仍能认出 `--settings`。
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

    assert!(requested(&[non_unicode(), os("--settings")]));
}

/// init→get 往返与未 init 默认值。
///
/// **全部 FLAGS 断言收进这一个用例**：FLAGS 是进程级 OnceLock，Rust 测试在同一
/// 二进制内并行执行——多个用例各自 init 会互相踩状态（OnceLock 首次定型后无法
/// 重置）。顺序：先断言未 init 的默认值，再 init true（定型后回不去 false）。
#[test]
fn init_then_get_roundtrip_and_default() {
    // 测试进程不经过 main：get 返回全 false 默认值，绝不 panic
    assert!(!get().settings);
    init(&[os("--settings")]);
    assert!(get().settings);
}
