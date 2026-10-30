//! 无文件层时的 panic 持久落点（评审缺口回归）：日志文件打不开 → 订阅器
//! 只有 stderr → GUI 构建下 panic 无落点。钩子必须在 `file_layer_ready()`
//! 为 false 时补写 bootstrap 文件。
//!
//! 独占测试二进制：panic 钩子与全局订阅器都是进程级单例。本二进制让
//! `open_log_file` 失败（在 base_dir 下放一个**同名文件** `logs`，目录创建
//! 必然失败），断言订阅器已装但文件层未接管，且 panic 报告落在固定
//! `%TEMP%/hinina-panic.log`。测试前后清理该文件，不向真实环境遗留内容。

use std::fs;
use std::path::PathBuf;

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "hinina-panic-no-file-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn bootstrap_path() -> PathBuf {
    std::env::temp_dir().join("hinina-panic.log")
}

/// 日志文件不可用：订阅器装上了（ready）但文件层未接管（!file_layer_ready），
/// 此时 panic 必须落到 bootstrap 文件——这是 GUI 构建下唯一的持久痕迹。
#[test]
fn panic_without_log_file_lands_in_bootstrap_file() {
    let dir = TempDir::new("no-file-layer");
    // 用同名文件占住 `logs` 路径：open_log_file 的 create_dir_all 必然失败
    fs::write(dir.0.join("logs"), b"not a directory").unwrap();
    std::env::set_var("RUST_LOG", "info");

    let logger = hinina_lib::infra::logger::Logger::init(&dir.0);
    assert!(
        !logger.has_log_file(),
        "logs/ 建不出来时文件层必须如实为不可用"
    );
    assert!(
        hinina_lib::infra::logger::ready(),
        "stderr-only 订阅器应已安装"
    );
    assert!(
        !hinina_lib::infra::logger::file_layer_ready(),
        "文件层未接管时该标志必须为 false（钩子据此补写持久文件）"
    );

    let bootstrap = bootstrap_path();
    let _ = fs::remove_file(&bootstrap);

    hinina_lib::infra::panic_report::install();
    let result = std::panic::catch_unwind(|| panic!("no-file-layer-probe"));
    assert!(result.is_err(), "panic 应照常 unwind，由 catch_unwind 收口");

    let content = fs::read_to_string(&bootstrap).expect("无文件层时 bootstrap 崩溃文件应存在");
    assert!(
        content.contains("no-file-layer-probe"),
        "消息应落盘：\n{content}"
    );
    assert!(
        content.contains("unix_epoch_secs:"),
        "应有时间戳头：\n{content}"
    );
    assert!(content.contains("堆栈:"), "应有堆栈段：\n{content}");

    let _ = fs::remove_file(&bootstrap);
}
