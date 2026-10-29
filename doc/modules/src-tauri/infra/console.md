# console

## 职责

`--console` 启动参数：release GUI 构建（`main.rs` 的 `windows_subsystem = "windows"`）重新附加调试控制台，让 panic 报告与日志的 stderr 路径可见。**只负责「开窗口」**：日志级别仍由 `RUST_LOG` 控制（release 默认 info），不因参数改变。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `requested` | `pub fn requested(args: &[String]) -> bool` | argv（**不含 argv[0]**，调用方负责 `skip(1)`）是否含精确的 `--console`；`--console=x` / `-console` 不算，其余参数静默忽略（未知参数不得炸掉启动） |
| `attach` | `pub fn attach()` | Windows：已有控制台（dev 构建 / 重复带参，`GetConsoleWindow` 判定）幂等返回 → `AttachConsole(ATTACH_PARENT_PROCESS)`（从终端启动 → 输出进该终端）→ 失败（双击启动无父控制台）则 `AllocConsole()` 新开窗口 → 两者都失败（如服务会话）静默保持 GUI 启动 → `CreateFileW("CONOUT$"/"CONIN$")` + `SetStdHandle` 重绑 std 三句柄。非 Windows：no-op |

## 直接依赖

- `windows-sys 0.59`（`cfg(windows)` only，Cargo.toml `[target.'cfg(windows)'.dependencies]`；features：`Win32_Foundation` / `Win32_Security` / `Win32_Storage_FileSystem` / `Win32_System_Console`。注意 `SetStdHandle` 与 `STD_*_HANDLE` 在 `Win32::System::Console`；`CreateFileW` 签名引用 `SECURITY_ATTRIBUTES`，被 `Win32_Security` feature 门控——签名取证自 windows-sys 0.59 源码）

## 被依赖

- `main.rs`（`main()` 第一件事：`requested` → `attach`；**必须早于 `Logger::init` 与任何 stdio 使用**——重绑的 std 句柄要被 panic 钩子的 stderr 兜底与 Logger 的 stderr 层从 `GetStdHandle` 取到）

## 逻辑流程

`main()` 解析 argv（`skip(1)`）→ `requested` → `attach`：`AttachConsole` 优先是为了「从终端启动时日志直接进该终端」；双击启动没有父控制台，落 `AllocConsole` 开新窗口（关掉这个窗口会杀进程，正常关闭应走应用窗口）。重绑句柄后，panic 钩子的 stderr 兜底与 `Logger::init` 的 stderr 层都落到控制台；不带参数时一切照旧（GUI 静默）。

## 测试

`src/infra/tests/console_tests.rs`：精确匹配、近形拒绝（`--consolex` / `-console` / `--console=1`）、混在其它参数中；非 Windows `attach` no-op 可编译。Windows 的 `attach` 涉及控制台 FFI 副作用，不进单测，由 release 人工验收覆盖（仓库根 `RELEASE.md` 清单）。
