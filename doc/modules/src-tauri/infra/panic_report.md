# panic_report

## 职责

进程级 panic 上报：panic 消息、线程、位置与完整堆栈写入持久文件。**保证：每次 panic 都无条件追加一份报告到 bootstrap 崩溃文件 `%TEMP%/hinina-panic.log`**——独立持久出口，不经 tracing 过滤器，也不依赖日志文件是否打开；**写入顺序同样属于保证：先 bootstrap、后 tracing/stderr**（后者存在 `SharedFileWriter` 重入阻塞窗口，见 `logger.md`，持久出口不能排在它之后）。日志未被过滤器关闭时同一条报告另经 `tracing::error!` 写入 `logs/hinina.log`（同时 stderr）。`RUST_LOG=off` / `panic=off` 会过滤 `error!`，`logs/` 目录权限不足、数据目录被占用会让文件层根本不接管——这些情况下 bootstrap 文件是唯一痕迹（「订阅器已装」不等于「日志在落盘」）。只有临时目录也不可写时崩溃才真正不可追查。GUI 子系统（release 构建，见 `main.rs` 的 `windows_subsystem`）下 stderr 无处可去，std 默认钩子的输出直接蒸发；插件 worker 的 `catch_unwind`（`plugin/manager.rs`）只兜插件 panic，主程序崩溃由此兜底——竞赛客户端最怕「闪退且无迹可寻」，本模块保证 panic 至少落一个持久文件。**启动期**（`startup_dialog` 已 arm 且启动未完成）的 panic 还会在持久记录之后弹一次系统错误框（见 `startup_dialog.md`）；运行期 panic 只落日志。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `install` | `pub fn install()` | `std::panic::set_hook` 安装进程级钩子。在 `main()` **最先**调用（早于参数解析、Runtime / Builder / Logger）——参数解析已改用 `args_os()`，本身**不会**因非 Unicode 参数 panic，钩子先装属**纵深防御**（历史上的 `env::args()` 会 panic，且 GUI 子系统下毫无输出）。重复调用只是替换 |
| `report` | `fn report(&PanicHookInfo)`（私有，钩子本体） | `payload_as_str()` 取消息（非字符串 payload 用 `<非字符串 panic payload>` 占位，不猜内容）+ 线程名 + panic 位置 + `Backtrace::force_capture()`（无视 RUST_BACKTRACE，崩溃取证要全量）。**先**无条件追加 bootstrap 文件（不经过滤器、不受文件层影响），**再**按 `logger::ready()` 走 `tracing::error!(target: "panic")`（过滤器可能丢弃它）或 stderr 兜底（写失败忽略）。顺序有意：tracing 文件层存在 `SharedFileWriter` 重入自死锁的理论窗口，持久出口排在它之后就会依赖该路径不挂起；先写 bootstrap 后，重入最多丢掉日志副本。是否被过滤器拦下无法在钩子里判定，故不做「文件层在就跳过」的优化——宁可重复不可丢失。**最后**调用 `startup_dialog::present_startup_panic`：启动期、主线程、本进程首次才弹；**只有 bootstrap 写入成功才把路径传给对话框**（否则正文说「未能写入」，不指向不存在的文件）；弹窗可能阻塞到用户点击，但记录已落盘，弹窗失败/阻塞都不影响持久化 |
| `write_bootstrap_report` | `fn write_bootstrap_report(path, message) -> bool`（私有） | 追加崩溃报告到 bootstrap 文件（**独立持久出口**，每次 panic 都写）：进程内用 `BOOTSTRAP_WRITE_LOCK`（**阻塞 `lock`**，中毒经 `into_inner` 取回）串行化「检查-截断-追加」——并发 panic **排队追加**，既不互相抹掉、也不因争用被跳过（`try_lock` 会在争用时整条丢弃，已移除；临界区无 panic 路径故不自死锁，代价是 I/O 挂起时 panic 线程等待）；每条以 `unix_epoch_secs` + `pid` 头行区分运行轮次与进程（多开实例时跨进程写入不串行，只能靠 pid 归因）；超过 1 MiB 先经 `logger::truncate_if_oversized` 轮转（与启动日志轮转共用同一份判据）；Unix 上拒绝符号链接目标、以 `0600` 创建并收敛已有文件权限；**所有写失败静默忽略**——崩溃路径绝不允许二次 panic。返回值只表示**本次追加是否成功**（符号链接拒绝、打开失败、`write_all` 失败均为 `false`），供启动对话框决定是否展示路径 |
| `bootstrap_log_path` | `pub fn bootstrap_log_path() -> PathBuf` | 固定 `%TEMP%/hinina-panic.log`：钩子运行时数据目录解析尚未发生（那正是可能 panic 的环节之一），任何「先解析再写」都会把失败面拉进钩子；公开给维护命令做体积预览与清理。集成测试经 `tests/common::SandboxTemp` 重定向 `TMPDIR`/`TMP`/`TEMP` 隔离；单元测试使用临时或注入路径（`local_data_usage_with` / 自带临时目录），不触碰真实崩溃文件 |
| `clear_bootstrap_log` | `pub fn clear_bootstrap_log() -> io::Result<u64>` | 删除崩溃文件并返回释放字节数（不存在为 0），与写入共用 `BOOTSTRAP_WRITE_LOCK`。供 `purge_local_data` 的「日志与崩溃记录」勾选项调用：该文件可能含插件可控文本，不能成为永久残留。删除失败返回错误由命令层如实回报。内部 `remove_file_and_size` 抽成纯函数供单测（测试不触碰真实 `%TEMP%`） |
| `format_report` | `fn format_report(payload, thread, location, backtrace) -> String`（私有） | 纯函数格式化（`PanicHookInfo` 无公开构造器，抽出供单测） |

## 直接依赖

- `std::backtrace::Backtrace` / `std::panic` / `std::io` / `std::fs`（stderr 与 bootstrap 文件都容忍写失败）
- `crate::infra::logger::ready`（只用于选择 tracing 还是 stderr；持久落点不依赖它）
- `crate::infra::startup_dialog::present_startup_panic`（启动期错误框；崩溃文件路径由本模块传入，避免模块环）
- `tracing`

## 被依赖

- `main.rs`（`main()` 最先调用 `install()`；不 import 其它业务模块）
- `commands::maintenance_cmd`（`local_data_usage` 读 `bootstrap_log_path` 体积；`purge_local_data` 随「日志与崩溃记录」调用 `clear_bootstrap_log`）
- `plugin::manager`（**策略交叉点**：插件 worker 有意不格式化 panic payload 以避开插件可控文本；全局钩子则记录**所有** panic 的 payload（含被 `catch_unwind` 捕获的插件 panic）。两处各自成立：插件日志走结构化字段，崩溃文件是本地取证且可由用户清理；钩子优先于捕获运行，属有意的全局可观测性）

## 逻辑流程

1. `main()` 启动 → `install()` 替换默认钩子（此时 `--console` 尚未处理：参数解析在钩子之后，用 `args_os()` 避免非 Unicode 参数 panic；console 附加完成后 stderr 兜底与 stderr 层才有控制台落点）。
2. 任意线程 panic（tokio worker、tauri 事件循环、命令处理器、插件 worker）→ 钩子**先于** `catch_unwind` 运行 → 格式化报告。
3. **先**无条件追加 `%TEMP%/hinina-panic.log`：`RUST_LOG=off` 或 `panic=off` 会过滤 `error!`，日志文件也可能打不开——bootstrap 是不受这些配置影响的出口；先写它也保证后续路径即使挂起（重入窗口）也不丢记录。
4. **再**按 `logger::ready()` 输出：已装订阅器 → `tracing::error!(target: "panic")`（GUI 子系统下 stderr 句柄无效、过滤器可能丢弃、`SharedFileWriter` 重入窗口理论上会阻塞——此时只丢日志副本，bootstrap 已有记录）；未装 → stderr 直写（写失败静默丢弃）。只有临时目录也不可写时崩溃才真正不可追查。
5. **最后**按 `startup_dialog` 的门控弹一次错误框：已 arm、启动未完成、主线程、本进程首次才弹；**只有第 3 步真正写入成功才把路径传给对话框**（临时目录不可写时正文说「未能写入」并保留 `--console` 提示）。对话框在独立线程执行并 `join`，避免主线程上的模态消息泵重入 Tauri 事件循环。它可能阻塞到用户点击，但第 3 步的记录已在磁盘上——弹窗不是持久化路径。
6. panic 照常 unwind：插件 worker 的 `catch_unwind`、tokio 的 `JoinError` 等既有恢复路径不受影响，只是先多了一份带堆栈的日志。

钩子经 tracing 文件层写日志的重入窗口已按源码排除：tracing-subscriber 先格式化进 thread-local 缓冲、再加锁写 writer（见 `logger.md` 设计要点）；bootstrap 写不经过文件层，无此窗口。bootstrap 路径在每次调用时重新解析 `std::env::temp_dir()`，测试通过重定向 `TMPDIR`/`TMP`/`TEMP` 隔离，不读写真实路径。

release 构建默认不带调试符号，堆栈以帧地址为主；需要可读符号时给 release profile 开 `debug` 即可，本模块不负责符号化。

## 测试

- `src/infra/tests/panic_report_tests.rs`：`format_report` 各段与占位；bootstrap 文件追加 + 时间戳/pid 头、超限截断、写失败静默；`remove_file_and_size` 返回删除前字节数且容忍文件不存在（清理路径，不触碰真实 `%TEMP%`）；**并发争用不跳过记录**（测试线程持锁 → 写入线程必须阻塞并在释放后落盘，`try_lock` 实现下必然失败）；Unix 下拒绝符号链接目标（不改写链接目标）。不在单测里安装钩子（全局钩子与 override 会泄漏到后续用例，端到端由独立集成二进制覆盖）。
- `src-tauri/tests/panic_report_bootstrap_tests.rs`（独立二进制，不初始化 logger）：`std::env::temp_dir()/hinina-panic.log` 的端到端证明，临时目录经 `tests/common` 的 `SandboxTemp` 重定向到用例沙箱。
- `src-tauri/tests/panic_report_no_file_layer_tests.rs`（独立二进制，模拟日志文件打不开）：用同名文件占住 `logs` 路径使 `open_log_file` 失败，断言 `ready()==true`、`has_log_file()==false`，且 panic 仍落 bootstrap 文件——GUI 构建下「无文件层就没有持久落点」的缺口回归。
- `src-tauri/tests/panic_report_rust_log_off_tests.rs`（独立二进制，`RUST_LOG=off`）：断言 `error!` 被过滤器拦下（日志文件里没有该 panic）时 bootstrap 仍有完整记录——`RUST_LOG` 不得让崩溃记录消失。
- `src-tauri/tests/panic_report_writes_log_tests.rs`（独立二进制，初始化真订阅器）：文件层正常的路径落 `logs/hinina.log` 的端到端证明，并断言 bootstrap 同时保留同一记录（独立出口不因日志正常而省略）。

四个 panic 报告集成测试二进制都独占进程：panic 钩子与全局订阅器都是进程级单例。它们**从不 `arm()` `startup_dialog`**，所以故意 panic 时不会弹窗（否则 CI 会卡在模态框）——这同时是启动期弹窗门控的隐式回归。
