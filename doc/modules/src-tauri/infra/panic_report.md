# panic_report

## 职责

进程级 panic 上报：panic 消息、线程、位置与完整堆栈写入持久文件。**保证：每次 panic 都无条件追加一份报告到 bootstrap 崩溃文件 `%TEMP%/hinina-panic.log`**——独立持久出口，不经 tracing 过滤器，也不依赖日志文件是否打开；日志未被过滤器关闭时同一条报告另经 `tracing::error!` 写入 `logs/hinina.log`（同时 stderr）。`RUST_LOG=off` / `panic=off` 会过滤 `error!`，`logs/` 目录权限不足、数据目录被占用会让文件层根本不接管——这些情况下 bootstrap 文件是唯一痕迹（「订阅器已装」不等于「日志在落盘」）。只有临时目录也不可写时崩溃才真正不可追查。GUI 子系统（release 构建，见 `main.rs` 的 `windows_subsystem`）下 stderr 无处可去，std 默认钩子的输出直接蒸发；插件 worker 的 `catch_unwind`（`plugin/manager.rs`）只兜插件 panic，主程序崩溃由此兜底——竞赛客户端最怕「闪退且无迹可寻」，本模块保证 panic 至少落一个持久文件。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `install` | `pub fn install()` | `std::panic::set_hook` 安装进程级钩子。在 `main()` **最先**调用（早于参数解析、Runtime / Builder / Logger）——参数解析用 `args_os()` 也可能 panic，必须落在钩子之后。重复调用只是替换 |
| `report` | `fn report(&PanicHookInfo)`（私有，钩子本体） | `payload_as_str()` 取消息（非字符串 payload 用 `<非字符串 panic payload>` 占位，不猜内容）+ 线程名 + panic 位置 + `Backtrace::force_capture()`（无视 RUST_BACKTRACE，崩溃取证要全量）。`logger::ready()` 为 true → `tracing::error!(target: "panic")`（过滤器可能丢弃它）；false → stderr 兜底（写失败忽略）。**两条路径最后都无条件**追加 bootstrap 文件——是否被过滤器拦下无法在钩子里判定，宁可重复不可丢失 |
| `write_bootstrap_report` | `fn write_bootstrap_report(path, message)`（私有） | 追加崩溃报告到 bootstrap 文件（**独立持久出口**，每次 panic 都写）：每条以 `unix_epoch_secs` 头行区分运行轮次；超过 1 MiB 先截断（rotate-by-truncate，与 logger 启动轮转同源）；**所有写失败静默忽略**——崩溃路径绝不允许二次 panic |
| `bootstrap_log_path` | `fn bootstrap_log_path() -> PathBuf`（私有） | 固定 `%TEMP%/hinina-panic.log`：钩子运行时数据目录解析尚未发生（那正是可能 panic 的环节之一），任何「先解析再写」都会把失败面拉进钩子；测试构建经 `set_bootstrap_path_override` 注入临时路径 |
| `format_report` | `fn format_report(payload, thread, location, backtrace) -> String`（私有） | 纯函数格式化（`PanicHookInfo` 无公开构造器，抽出供单测） |

## 直接依赖

- `std::backtrace::Backtrace` / `std::panic` / `std::io` / `std::fs`（stderr 与 bootstrap 文件都容忍写失败）
- `crate::infra::logger::ready`（只用于选择 tracing 还是 stderr；持久落点不依赖它）
- `tracing`

## 被依赖

- `main.rs`（`main()` 最先调用 `install()`；不 import 其它业务模块）

## 逻辑流程

1. `main()` 启动 → `install()` 替换默认钩子（此时 `--console` 尚未处理：参数解析在钩子之后，用 `args_os()` 避免非 Unicode 参数 panic；console 附加完成后 stderr 兜底与 stderr 层才有控制台落点）。
2. 任意线程 panic（tokio worker、tauri 事件循环、命令处理器、插件 worker）→ 钩子**先于** `catch_unwind` 运行 → 格式化报告。
3. `logger::ready()`（订阅器已装）→ `tracing::error!(target: "panic")`；否则 stderr 直写（GUI 子系统下句柄无效时静默丢弃）。
4. **无论上一步走哪条**，都追加 `%TEMP%/hinina-panic.log`：`RUST_LOG=off` 或 `panic=off` 会过滤 `error!`，日志文件也可能打不开——bootstrap 是唯一不受这些配置影响的出口。只有临时目录也不可写时崩溃才真正不可追查。
5. panic 照常 unwind：插件 worker 的 `catch_unwind`、tokio 的 `JoinError` 等既有恢复路径不受影响，只是先多了一份带堆栈的日志。

钩子经 tracing 文件层写日志存在 `SharedFileWriter` 重入自死锁的理论窗口（见 `logger.md` 设计要点）；bootstrap 写不经过文件层，无此窗口。bootstrap 路径在每次调用时重新解析 `std::env::temp_dir()`，测试通过重定向 `TMPDIR`/`TMP`/`TEMP` 隔离，不读写真实路径。

release 构建默认不带调试符号，堆栈以帧地址为主；需要可读符号时给 release profile 开 `debug` 即可，本模块不负责符号化。

## 测试

- `src/infra/tests/panic_report_tests.rs`：`format_report` 各段与占位；bootstrap 文件追加 + 时间戳头、超限截断、写失败静默；钩子全链路（经 `set_bootstrap_path_override` 注入临时路径，`ready()` 恒 false 的测试构建走 bootstrap 分支）落盘消息/时间戳/堆栈。
- `src-tauri/tests/panic_report_bootstrap_tests.rs`（独立二进制，不初始化 logger）：`std::env::temp_dir()/hinina-panic.log` 的端到端证明，临时目录经 `tests/common` 的 `SandboxTemp` 重定向到用例沙箱。
- `src-tauri/tests/panic_report_no_file_layer_tests.rs`（独立二进制，模拟日志文件打不开）：用同名文件占住 `logs` 路径使 `open_log_file` 失败，断言 `ready()==true`、`has_log_file()==false`，且 panic 仍落 bootstrap 文件——GUI 构建下「无文件层就没有持久落点」的缺口回归。
- `src-tauri/tests/panic_report_rust_log_off_tests.rs`（独立二进制，`RUST_LOG=off`）：断言 `error!` 被过滤器拦下（日志文件里没有该 panic）时 bootstrap 仍有完整记录——`RUST_LOG` 不得让崩溃记录消失。
- `src-tauri/tests/panic_report_writes_log_tests.rs`（独立二进制，初始化真订阅器）：文件层正常的路径落 `logs/hinina.log` 的端到端证明，并断言 bootstrap 同时保留同一记录（独立出口不因日志正常而省略）。

四个 panic 报告集成测试二进制都独占进程：panic 钩子与全局订阅器都是进程级单例。
