//! `--console` 启动参数：release GUI 构建重新附加调试控制台。
//!
//! `windows_subsystem = "windows"`（见 main.rs）让 release 不再附带控制台，
//! stderr 路径不可见；带 `--console` 启动时先尝试挂到**父进程**控制台（从
//! 终端启动 → 输出直接进该终端），失败（双击启动）再分配新控制台窗口，并
//! 重绑 std 句柄。重绑必须早于 `Logger::init` 与任何 stdio 使用——tracing 的
//! stderr 层与 panic 钩子的兜底写都从 `GetStdHandle` 取句柄。
//!
//! 非 Windows 平台本就带终端，`attach` 为 no-op；级别语义不变（仍由
//! `RUST_LOG` 控制），本参数只负责「开窗口」。

/// argv（**不含 argv[0]**，调用方负责 `skip(1)`）是否请求调试控制台。
///
/// 精确匹配 `--console`：`--console=x`、`-console` 等一律不算；其余参数
/// 静默忽略，留给未来扩展（避免未知参数把启动炸掉）。入参用 `OsString`：
/// `std::env::args()` 遇非 Unicode 参数会 panic，`args_os()` 不会。
pub fn requested(args: &[std::ffi::OsString]) -> bool {
    args.iter()
        .any(|arg| arg.as_os_str() == std::ffi::OsStr::new("--console"))
}

/// 按需附加调试控制台。Windows 实现见下；其它平台 no-op。
#[cfg(windows)]
pub fn attach() {
    // SAFETY: 仅调用 kernel32 控制台 API；句柄替换发生在任何 stdio 使用之前
    // （main 最早期），不与其它代码竞争 std 句柄。
    unsafe { attach_windows() }
}

/// 非 Windows 平台没有「隐藏的控制台」需要恢复，no-op。
#[cfg(not(windows))]
pub fn attach() {}

#[cfg(windows)]
unsafe fn attach_windows() {
    use windows_sys::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Console::{
        AllocConsole, AttachConsole, GetConsoleWindow, SetStdHandle, ATTACH_PARENT_PROCESS,
        STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };

    // 已有控制台（dev 构建 / 重复带参）就不动：重复 AllocConsole 会开出第二个窗口
    if !GetConsoleWindow().is_null() {
        return;
    }
    // 先挂父进程控制台（从终端启动 → 输出进该终端）；失败（双击启动，无父控制台）
    // 再新开窗口。两者都失败（如服务会话）：保持 GUI 静默启动，不阻断
    if AttachConsole(ATTACH_PARENT_PROCESS) == 0 && AllocConsole() == 0 {
        return;
    }

    // 重绑 std 句柄：GUI 进程的 std 句柄无效，std::io 每次经 GetStdHandle 取句柄。
    // CONOUT$/CONIN$ 是新控制台的设备名，CreateFileW 拿到的句柄才有效。
    let conout = wide("CONOUT$");
    let out = CreateFileW(
        conout.as_ptr(),
        GENERIC_WRITE,
        FILE_SHARE_WRITE,
        std::ptr::null(),
        OPEN_EXISTING,
        0,
        std::ptr::null_mut(),
    );
    if out != INVALID_HANDLE_VALUE {
        SetStdHandle(STD_OUTPUT_HANDLE, out);
        SetStdHandle(STD_ERROR_HANDLE, out);
    }
    let conin = wide("CONIN$");
    let input = CreateFileW(
        conin.as_ptr(),
        GENERIC_READ,
        FILE_SHARE_READ,
        std::ptr::null(),
        OPEN_EXISTING,
        0,
        std::ptr::null_mut(),
    );
    if input != INVALID_HANDLE_VALUE {
        SetStdHandle(STD_INPUT_HANDLE, input);
    }
}

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
#[path = "tests/console_tests.rs"]
mod tests;
