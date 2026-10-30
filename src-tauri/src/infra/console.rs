//! `--console` 启动参数：release GUI 构建重新附加调试控制台。
//!
//! `windows_subsystem = "windows"`（见 main.rs）让 release 不再附带控制台，
//! stderr 路径不可见；带 `--console` 启动时先尝试挂到**父进程**控制台（从
//! 终端启动 → 输出直接进该终端），失败（双击启动）再分配新控制台窗口。**无论
//! Attach 还是 Alloc，新取得控制台后都重绑 std 句柄**——重绑的是本进程的
//! 句柄，不改控制台自身的设置。重绑必须早于 `Logger::init` 与任何 stdio 使用
//! ——tracing 的 stderr 层与 panic 钩子的兜底写都从 `GetStdHandle` 取句柄。
//!
//! 非 Windows 平台本就带终端，`attach` 为 no-op；级别语义不变（仍由
//! `RUST_LOG` 控制）。显示配置（UTF-8 代码页 + VT 处理，见 `configure_console`）
//! **只在 `AllocConsole` 新开窗口时执行**——那是本参数自己开出来的会话；挂到
//! 父终端时不改显示配置（那是用户日常会话的状态）。调用 `attach` 前就已存在
//! 控制台的路径（dev 终端、重复带参）则完全不重绑、不改动。

/// argv（**不含 argv[0]**，调用方负责 `skip(1)`）是否请求调试控制台。
///
/// 精确匹配 `--console`：`--console=x`、`-console` 等一律不算；其余参数
/// 静默忽略，留给未来扩展（避免未知参数把启动炸掉）。入参用 `OsString`：
/// `std::env::args()` 遇非 Unicode 参数会 panic，`args_os()` 不会。
pub fn requested(args: &[std::ffi::OsString]) -> bool {
    // OsString: PartialEq<str>：与非 Unicode 参数共存也不会 panic
    args.iter().any(|arg| arg == "--console")
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

/// 控制台输出代码页：UTF-8。
///
/// std 按 UTF-8 字节写控制台，而控制台用**自己的**代码页把字节解码成字符；
/// 中文系统默认 936（GBK），UTF-8 字节被当成 GBK 解码就是满屏乱码
/// （`Hinina 启动中...` 显示成 `Hinina 鍚...`）。
#[cfg(windows)]
const UTF8_CODEPAGE: u32 = 65001;

#[cfg(windows)]
unsafe fn attach_windows() {
    use windows_sys::Win32::System::Console::{
        AllocConsole, AttachConsole, GetConsoleWindow, ATTACH_PARENT_PROCESS,
    };

    if GetConsoleWindow().is_null() {
        // 先挂父进程控制台（从终端启动 → 输出进该终端）；失败（双击启动，无父控制台）
        // 再新开窗口。两者都失败（如服务会话）：保持 GUI 静默启动，不阻断
        let attached_to_parent = AttachConsole(ATTACH_PARENT_PROCESS) != 0;
        if !attached_to_parent && AllocConsole() == 0 {
            return;
        }
        rebind_std_handles();
        // 只在 **AllocConsole 新开**的窗口上做输出配置：那是本参数自己开出来的
        // 会话，随窗口销毁，不碰用户日常终端（见 configure_console 的副作用边界）。
        // AttachConsole 挂到父终端时不配置：切父终端的代码页等于替用户执行
        // `chcp 65001`，会影响该终端里其它按原代码页显示输出的程序，且退出不回滚。
        if !attached_to_parent {
            configure_console();
        }
    }
    // 已有控制台（dev 构建 / 从已有终端重复带参）：不重绑、不改配置——那个
    // 控制台属于用户的日常会话。窗口由启动者负责配置（Windows Terminal 默认
    // UTF-8 + VT；老 conhost 若显示为乱码，属启动环境自身设置）。
}

/// 重绑 std 三句柄。
///
/// GUI 进程的 std 句柄无效，`std::io` 每次经 `GetStdHandle` 取句柄；
/// `CONOUT$`/`CONIN$` 是当前控制台的设备名，`CreateFileW` 拿到的句柄才有效。
/// Attach/Alloc 任一成功后都执行——重绑只动本进程句柄，不改控制台自身设置。
#[cfg(windows)]
unsafe fn rebind_std_handles() {
    use windows_sys::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Console::{
        SetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };

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

/// 让控制台按本进程「想要的语义」渲染输出：UTF-8 代码页 + VT 转义序列。
///
/// 只开窗口不改这两项，`--console` 窗口里的日志就是坏的：
/// - 代码页不是 UTF-8 → 中文乱码；
/// - 没开 `ENABLE_VIRTUAL_TERMINAL_PROCESSING` → ANSI 序列不被解释，ESC 被丢，
///   只剩字面量 `[32m[INFO][0m`（stderr 层默认开着色）。
///
/// **副作用边界（有意收窄）**：只在 `AllocConsole` **新开**的窗口调用——那是
/// 本参数自己开出来的会话，随窗口销毁。**挂父控制台（从终端带参启动）绝不调用**：
/// 切父终端的代码页等于替用户执行 `chcp 65001`，会影响该终端里其它按原代码页
/// 显示输出的程序，且退出不回滚——用户日常习惯不由调试开关改变。所有调用失败
/// 都只让输出难看，不阻断启动（VT 需要 Windows 10 1511+）。
#[cfg(windows)]
unsafe fn configure_console() {
    use windows_sys::Win32::Foundation::{
        CloseHandle, GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Console::{
        GetConsoleMode, SetConsoleMode, SetConsoleOutputCP, ENABLE_PROCESSED_OUTPUT,
        ENABLE_VIRTUAL_TERMINAL_PROCESSING,
    };

    SetConsoleOutputCP(UTF8_CODEPAGE);

    // 配置用独立句柄：std 输出句柄有写权限就够，改它的访问权限会牵动上面那条
    // 已验证的「std 只管写」路径；读描述符状态按惯例带读权限开。
    let conout = wide("CONOUT$");
    let handle = CreateFileW(
        conout.as_ptr(),
        GENERIC_READ | GENERIC_WRITE,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        std::ptr::null(),
        OPEN_EXISTING,
        0,
        std::ptr::null_mut(),
    );
    if handle == INVALID_HANDLE_VALUE {
        return;
    }

    let mut mode = 0;
    if GetConsoleMode(handle, &mut mode) != 0 {
        // ENABLE_PROCESSED_OUTPUT 是 VT 处理的前置条件；老控制台默认已开，显式补上
        let _ = SetConsoleMode(
            handle,
            mode | ENABLE_PROCESSED_OUTPUT | ENABLE_VIRTUAL_TERMINAL_PROCESSING,
        );
    }
    let _ = CloseHandle(handle);
}

/// UTF-16 + NUL 编码，供 Win32 `*W` API 使用。
///
/// `startup_dialog` 的 `MessageBoxW` 与本模块的控制台句柄共用同一实现，
/// 避免两份编码细节漂移。
#[cfg(windows)]
pub(crate) fn wide(value: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
#[path = "tests/console_tests.rs"]
mod tests;
