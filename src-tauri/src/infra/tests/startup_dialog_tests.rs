use super::*;
use std::path::Path;
use std::sync::atomic::AtomicBool;

#[test]
fn message_shows_crash_path_when_it_was_written() {
    let text = message(
        "无法初始化应用数据：创建 base_dir 失败: 拒绝访问",
        Some(Path::new(r"C:\Temp\hinina-panic.log")),
    );

    assert!(text.contains("Hinina 启动失败"));
    assert!(text.contains("无法初始化应用数据：创建 base_dir 失败: 拒绝访问"));
    assert!(text.contains(r"C:\Temp\hinina-panic.log"));
    assert!(text.contains("--console"));
    assert!(text.contains("关闭此对话框后程序将退出"));
}

/// 崩溃记录没写进去（临时目录不可写）时不能再把用户指向那个文件。
#[test]
fn message_admits_the_crash_record_was_not_written() {
    let text = message("无法启动桌面应用：WebView2 不可用", None);

    assert!(text.contains("无法启动桌面应用：WebView2 不可用"));
    assert!(text.contains("崩溃记录未能写入"));
    assert!(!text.contains("崩溃详情："));
    assert!(text.contains("--console"));
}

#[test]
fn only_an_unstarted_main_thread_presents() {
    assert!(should_present(false, true));

    // 启动已完成：运行期 panic 只落日志
    assert!(!should_present(true, true));

    // 未 arm（主线程 ID 未记录）或后台线程：都不弹
    assert!(!should_present(false, false));
}

#[test]
fn presentation_is_claimed_once() {
    let presented = AtomicBool::new(false);

    assert!(claim_presentation(&presented));
    assert!(!claim_presentation(&presented));
}
