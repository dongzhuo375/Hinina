// `app_cmd::validate_external_url` 的纯函数单测：scheme 白名单与令牌安全。
//
// 真实 `open_external`（AppHandle + 插件 Rust API）不做单测——tauri mock runtime
// 会污染测试二进制（single_instance 先例），行为靠真机人工验收；
// 本文件只锁定「哪些 URL 能进 opener」这道闸门。

use super::app_cmd::validate_external_url;

#[test]
fn accepts_http_and_https_urls() {
    for url in [
        "https://errata.example.com/a1",
        "http://hoj.example.com/problem/1166",
        "https://hydro.ac/p/UxKd9/",
    ] {
        let parsed = validate_external_url(url).unwrap_or_else(|e| panic!("{url}: {e}"));
        assert_eq!(parsed.as_str(), url);
    }
}

#[test]
fn rejects_non_http_schemes() {
    for url in [
        "file:///C:/Windows/System32/calc.exe",
        "javascript:alert(1)",
        "data:text/html,<script>alert(1)</script>",
        "ftp://example.com/file",
        "chrome://settings",
        "\\\\server\\share\\x",
    ] {
        assert!(validate_external_url(url).is_err(), "{url} 必须被拒绝");
    }
}

#[test]
fn rejects_malformed_urls() {
    for url in ["", "   ", "not a url at all", "http://"] {
        assert!(validate_external_url(url).is_err(), "{url:?} 必须被拒绝");
    }
}

/// 题面/公告的链接 query 可能带签名令牌：错误信息进日志/UI，
/// **绝不能回显原始 URL**（globalErrorGuard「不记完整 URL」同款先例）。
#[test]
fn error_message_never_contains_the_url() {
    let url = "ftp://example.com/?token=SECRET";
    let err = validate_external_url(url).unwrap_err();
    assert!(!err.contains("SECRET"), "错误信息不得回显 URL: {err}");
    assert!(!err.contains(url), "错误信息不得回显 URL: {err}");
}
