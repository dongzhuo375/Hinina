use super::has_flag;
use std::ffi::OsString;

fn os(value: &str) -> OsString {
    OsString::from(value)
}

#[test]
fn matches_exact_argument_only() {
    assert!(has_flag(&[os("--settings")], "--settings"));
    assert!(!has_flag(&[], "--settings"));
    assert!(!has_flag(&[os("--settingsx")], "--settings"));
    assert!(!has_flag(&[os("-settings")], "--settings"));
    assert!(!has_flag(&[os("--settings=1")], "--settings"));
    // 同一输入对不同 flag 的判定互不影响
    assert!(has_flag(&[os("--console"), os("--settings")], "--console"));
    assert!(!has_flag(&[os("--console")], "--settings"));
}

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

    assert!(has_flag(&[non_unicode(), os("--settings")], "--settings"));
}
