# console

## 职责

`--console` 启动参数：release GUI 构建（`main.rs` 的 `windows_subsystem = "windows"`）重新附加调试控制台，让 panic 报告与日志的 stderr 路径可见。负责两件事，边界不同：**拿到控制台时重绑本进程的 std 句柄**（Attach/Alloc 一样，只动自己进程，不碰控制台设置）；**只有本参数 `AllocConsole` 新开出来的窗口才配置显示**（UTF-8 代码页 + VT）。日志级别仍由 `RUST_LOG` 控制（release 默认 info），不因参数改变。参数解析本身不 panic：入参用 `OsString`，非 Unicode 参数不会触发 `std::env::args()` 的 panic。

「正确显示」是两件缺一不可的事，二者都是**控制台会话级**状态：

- **输出代码页切 UTF-8**（65001）：std 按 UTF-8 字节写控制台，控制台却用**自己的**代码页解码；中文系统默认 936（GBK），不改就是满屏乱码（`Hinina 启动中...` 显示成 `Hinina 鍚...`）。
- **打开 `ENABLE_VIRTUAL_TERMINAL_PROCESSING`**：stderr 层默认开着色，不开 VT 时转义序列不被解释、ESC 被丢弃，只剩字面量 `[32m[INFO][0m`。

**作用范围有意收窄**：这两项只在**新开窗口**（双击启动的 `AllocConsole`）时配置——那是本参数自己开出来的会话，随窗口销毁，不碰用户日常终端。**从终端带参启动（挂父控制台）与 dev 构建一律不改配置**：切父终端的代码页等于替用户执行 `chcp 65001`，会影响该终端里其它按原代码页显示输出的程序，且退出不回滚——用户日常习惯不由调试开关改变。两种配置失败都只让输出难看，不阻断启动（VT 需要 Windows 10 1511+）。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `requested` | `pub fn requested(args: &[std::ffi::OsString]) -> bool` | argv（**不含 argv[0]**，调用方用 `args_os().skip(1)` 收集）是否含精确的 `--console`；比较用 `OsString == "--console"`（std 提供 `PartialEq<str>`）。`--console=x` / `-console` 不算，其余参数静默忽略（未知参数不得炸掉启动） |
| `attach` | `pub fn attach()` | Windows：已有控制台（`GetConsoleWindow` 判定，dev 构建 / 重复带参）幂等跳过——不重绑、不改配置，与不带参一致。没有控制台才去拿一个：`AttachConsole(ATTACH_PARENT_PROCESS)`（从终端启动 → 输出进该终端）优先，失败（双击启动无父控制台）则 `AllocConsole()` 新开窗口，两者都失败（如服务会话）静默保持 GUI 启动。**Attach/Alloc 任一成功都 `rebind_std_handles`**（重绑的是本进程的 std 句柄，不触碰控制台自身设置）；**只有 `AllocConsole` 新开的窗口**才 `configure_console`——父终端不改显示配置（那是用户日常会话的状态）。非 Windows：no-op |
| `rebind_std_handles` | `unsafe fn rebind_std_handles()`（`cfg(windows)`） | `CreateFileW("CONOUT$"/"CONIN$")` + `SetStdHandle` 重绑 std 三句柄：GUI 进程的 std 句柄无效，`std::io` 每次经 `GetStdHandle` 取句柄。Attach/Alloc **新取得**控制台时执行（重绑的是本进程句柄，不改控制台自身设置） |
| `configure_console` | `unsafe fn configure_console()`（`cfg(windows)`） | `SetConsoleOutputCP(UTF8_CODEPAGE)` + 在 `CONOUT$` 句柄上 `GetConsoleMode` → 叠加 `ENABLE_PROCESSED_OUTPUT \| ENABLE_VIRTUAL_TERMINAL_PROCESSING` → `SetConsoleMode`。配置用独立句柄（带读权限），std 输出句柄维持「只有写权限」的原路径不变。**只由 `AllocConsole` 新开的窗口调用**，见「职责」的作用范围 |
| `UTF8_CODEPAGE` | `const UTF8_CODEPAGE: u32 = 65001`（`cfg(windows)`） | 控制台输出代码页 utf-8 |
| `wide` | `pub(crate) fn wide(value: &str) -> Vec<u16>`（`cfg(windows)`） | UTF-16 + NUL 编码，供本模块的控制台句柄与 `startup_dialog` 的 `MessageBoxW` 共用，避免两份编码细节漂移 |

## 直接依赖

- `std::ffi::OsString`（参数比较，兼容非 Unicode argv）
- `windows-sys 0.59`（`cfg(windows)` only，Cargo.toml `[target.'cfg(windows)'.dependencies]`；features：`Win32_Foundation` / `Win32_Security` / `Win32_Storage_FileSystem` / `Win32_System_Console`。注意 `SetStdHandle` / `STD_*_HANDLE` / `SetConsoleOutputCP` / `GetConsoleMode` / `SetConsoleMode` 与 `ENABLE_*` 常量都在 `Win32::System::Console`，`CloseHandle` 在 `Win32::Foundation`；`CreateFileW` 签名引用 `SECURITY_ATTRIBUTES`，被 `Win32_Security` feature 门控——签名取证自 windows-sys 0.59 源码）

## 被依赖

- `main.rs`（`main()` 的**第二**步：先安装 panic 钩子（`infra::panic_report::install`），再用 `args_os().skip(1)` 收集参数并 `requested` → `attach`；`attach` **必须早于 `Logger::init` 与任何 stdio 使用**——重绑的 std 句柄要被 panic 钩子的 stderr 兜底与 Logger 的 stderr 层从 `GetStdHandle` 取到，新窗口的代码页与 VT 也要在第一批日志写出**之前**生效。钩子先装是纵深防御：重绑之前的任何 panic 也有落点）

## 逻辑流程

`main()` 先装 panic 钩子 → `args_os().skip(1)` 收集参数（非 Unicode 参数不会 panic）→ `requested` 精确匹配 `--console` → `attach`：
- **双击启动**（无父控制台）：`AllocConsole` 新开窗口 → `rebind_std_handles` 重绑句柄 → `configure_console` 切 UTF-8/开 VT——这个窗口是本参数开的，随窗口销毁，显示保证完整。
- **从终端带参启动**（挂父控制台成功）：重绑句柄（`AttachConsole` 挂上的控制台对本进程同样是「新取得」，`CONOUT$`/`CONIN$` 句柄有效）、**不配置**——终端是用户的日常会话，代码页与 VT 属于它自己的设置；输出按该终端当前代码页渲染（Windows Terminal 默认 UTF-8 + VT，显示正常；老 conhost 若为 936 且未开 VT，中文与着色会显示为坏，属启动环境自身设置，应用不改它）。
- **已有控制台**（dev 构建 / 重复带参）：幂等跳过，不重绑、不配置，与不带参一致。
- 不带参数时一切照旧（GUI 静默）。

panic 钩子的 stderr 兜底与 `Logger::init` 的 stderr 层在两种路径下都落得到控制台；「显示得对」只在新窗口承诺。

## 测试

`src/infra/tests/console_tests.rs`：精确匹配、近形拒绝（`--consolex` / `-console` / `--console=1`）、混在其它参数中、非 Unicode 参数（Unix 非法字节 / Windows 未配对代理项）下仍能识别 `--console`；非 Windows `attach` no-op 可编译。Windows 的 `attach` / `configure_console` 涉及控制台 FFI 副作用，不进单测，由 release 人工验收覆盖（仓库根 `RELEASE.md` 清单：双击 + `--console` 新开窗口内「中文日志可读、ANSI 着色生效」；从终端启动仅要求输出落进该终端、不要求也不承诺改其代码页）。

**已取证现象**（2026-10-30，本机一次性探针 `doc/local/bug/console_probe.py`（gitignored），在同一真实控制台上 A/B 写同一串 `中文 + ESC[32mX ESC[0m` 字节后 `ReadConsoleOutputCharacterW` 读回屏幕缓冲区首行）：默认控制台 `GetConsoleOutputCP()=936`、mode=0x0003，首行读回 `涓`（U+6D93）、U+E15F（私用区，屏幕显示为方框）、`枃`（U+6783），且转义序列以字面量（U+001B、`[`、`3`、`2`、`m`…）留在屏上；切 65001 并开 VT（mode=0x0007）后读回 `中文 X`（着色生效、无字面量）。A 组读回即「UTF-8 字节按 GBK 误解码」的结果；探针以首行宽度截断，转义序列完整形态见截图 `doc/local/bug/console.png`。
