# panic_report

## 职责

进程级 panic 上报：panic 消息、线程、位置与完整堆栈写进现有 logger（`logs/hinina.log`）。GUI 子系统（release 构建，见 `main.rs` 的 `windows_subsystem`）下 stderr 无处可去，std 默认钩子的输出直接蒸发；插件 worker 的 `catch_unwind`（`plugin/manager.rs`）只兜插件 panic，主程序崩溃此前无任何落点——竞赛客户端最怕「闪退且无迹可寻」，本模块补齐「崩溃可追查」。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `install` | `pub fn install()` | `std::panic::set_hook` 安装进程级钩子。在 `main()` 最早期调用（早于 Runtime / Builder / Logger）——越早安装，启动期 panic 越有迹可循。重复调用只是替换 |
| `report` | `fn report(&PanicHookInfo)`（私有，钩子本体） | `payload_as_str()` 取消息（非字符串 payload 用 `<非字符串 panic payload>` 占位，不猜内容）+ 线程名 + panic 位置 + `Backtrace::force_capture()`（无视 RUST_BACKTRACE，崩溃取证要全量）→ `tracing::error!(target: "panic")`；`logger::ready()` 为 false（订阅器未装）时降级为写失败可忽略的 stderr 直写。钩子自身不做任何可能 panic 的操作；若钩子内仍 panic，std 直接 abort（最后防线，好过无限递归） |
| `format_report` | `fn format_report(payload, thread, location, backtrace) -> String`（私有） | 纯函数格式化（`PanicHookInfo` 无公开构造器，抽出供单测） |

## 直接依赖

- `std::backtrace::Backtrace` / `std::panic` / `std::io`（写失败可忽略的 stderr 直写）
- `crate::infra::logger::ready`（分流输出路径）
- `tracing`

## 被依赖

- `main.rs`（`main()` 最早期调用 `install()`；不 import 其它业务模块）

## 逻辑流程

1. `main()` 启动 → `install()` 替换默认钩子（此时 `--console` 已处理完控制台附加，stderr 兜底与 stderr 层才有落点）。
2. 任意线程 panic（tokio worker、tauri 事件循环、命令处理器、插件 worker）→ 钩子**先于** `catch_unwind` 运行 → 格式化报告。
3. `logger::ready()`（订阅器已装）→ `tracing::error!(target: "panic")` 双路输出，落 `logs/hinina.log`；未就绪 → stderr 直写（GUI 子系统下句柄无效时静默丢弃）。
4. panic 照常 unwind：插件 worker 的 `catch_unwind`、tokio 的 `JoinError` 等既有恢复路径不受影响，只是先多了一份带堆栈的日志。

release 构建默认不带调试符号，堆栈以帧地址为主；需要可读符号时给 release profile 开 `debug` 即可，本模块不负责符号化。

## 测试

- `src/infra/tests/panic_report_tests.rs`：`format_report` 各段齐全与占位标注；`install` + `catch_unwind` 全链路不 abort（单元构建无全局订阅器，走 stderr 兜底分支）。
- `src-tauri/tests/panic_report_writes_log_tests.rs`（独立二进制：钩子与全局订阅器都是进程级单例，必须隔离）：真订阅器写临时目录 → 触发 panic → 断言日志含消息、真实触发位置与编号堆栈帧——「闪退可追查」的端到端回归闸门，CI 两个 OS 都跑。
