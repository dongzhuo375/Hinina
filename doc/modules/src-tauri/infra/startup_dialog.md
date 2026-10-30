# startup_dialog

## 职责

启动期致命错误的系统错误对话框（panic 钩子的 UI 出口）。`main()` 在 panic 钩子安装后 `arm()` 记录主线程；Tauri 发出 `RunEvent::Ready`（窗口创建与 setup 均已完成）时 `mark_started()`。两者之间主线程上的 panic 视为启动失败，钩子在崩溃记录落盘**之后**调用 `present_startup_panic` 弹一次 `MessageBoxW`。覆盖三处启动 `expect`（Tokio runtime / Tauri Builder / AppContext）与 Tauri 自身创建窗口失败（WebView2 缺失等，tauri 的 `panic!("Failed to setup app: ...")`）。运行期 panic 只落日志，不弹窗。非 Windows 是 no-op（产品只发 Windows，开发/CI 的 stderr 已能看到 panic 报告）。

**为什么标记点选 `RunEvent::Ready`**：Tauri 2.11 的 `fn setup` 先按 `tauri.conf.json` 的 `app.windows` 创建窗口（WebView2 在这一步初始化），再调用用户的 setup 闭包；`RunEvent::Ready` 在 setup 返回后才发给 `run` 回调（tauri `app.rs` 的 `fn setup` / `make_run_event_loop_callback`）。窗口/WebView2 创建失败会在 setup 内 panic，此时 `mark_started` 尚未执行，错误框照常弹出——标记点不依赖用户 setup 闭包内部的顺序。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `arm` | `pub fn arm()` | 记录主线程 `ThreadId`（未记录即未启用）。`main()` 在 `panic_report::install()` 之后调用。**测试构建永不调用**：模态框会卡死 CI（`panic_report_*` 集成测试会故意 panic） |
| `mark_started` | `pub fn mark_started()` | 标记启动完成：`main.rs` 的 `run` 回调收到 `RunEvent::Ready` 时调用。此后 panic 不再弹窗，运行期崩溃只走日志与崩溃文件 |
| `present_startup_panic` | `pub fn present_startup_panic(payload: &str, crash_log: Option<&Path>)` | 钩子的调用点。只有「启动未完成、主线程、本进程首次」才弹；正文含 payload（三处 expect 已改成中文上下文）、「关闭后程序将退出」、`--console` 提示。**`crash_log` 只在崩溃记录真正写入成功时是 `Some`**；`None` 时正文明确说「崩溃记录未能写入（临时目录不可写）」，不展示不存在的路径。`MessageBoxW` 在独立线程执行并 `join`——钩子可能跑在 Tauri 事件循环回调里，主线程上嵌套模态消息泵会重入事件循环。线程创建/FFI 失败静默跳过（持久记录已先写入） |
| `should_present` | `fn should_present(started, on_main) -> bool`（私有） | 纯判定：启动未完成且在主线程。未 `arm` 时 `on_main` 必为 false，故不需要单独的 armed 标志 |
| `claim_presentation` | `fn claim_presentation(&AtomicBool) -> bool`（私有） | 一次性领取弹窗权：并发 panic 只有一个线程弹出 |
| `message` | `fn message(payload: &str, crash_log: Option<&Path>) -> String`（私有） | 正文格式化纯函数，供单测 |

## 直接依赖

- `windows-sys 0.59`（`cfg(windows)`，Cargo.toml `[target.'cfg(windows)'.dependencies]`；feature `Win32_UI_WindowsAndMessaging`：`MessageBoxW` / `MB_OK` / `MB_ICONERROR` / `MB_SETFOREGROUND` / `MB_TOPMOST`，`HWND` 为空指针表示无父窗口。签名取证自 windows-sys 0.59 源码）
- `crate::infra::console::wide`（UTF-16 + NUL 编码，与控制台句柄共用同一实现）
- `std::sync::atomic`（started / presented 两个进程级标志）
- `std::sync::OnceLock` + `std::thread::ThreadId`（主线程判定）
- `std::path::Path`（崩溃文件路径由调用方传入，**不依赖 `panic_report`**，避免 `panic_report ↔ startup_dialog` 模块环）

## 被依赖

- `main.rs`（`main()` 里的 `arm`；`run` 回调的 `RunEvent::Ready` 里的 `mark_started`）
- `infra::panic_report::report`（启动期 panic 的钩子末尾调用，顺序见 `panic_report.md`）

## 逻辑流程

1. `main()` 装 panic 钩子 → `startup_dialog::arm()`（记主线程）→ 参数解析 / `--console` / Tokio runtime / Tauri Builder。
2. 启动失败 → panic：三处 expect 已改为 `unwrap_or_else(|e| panic!("中文上下文：{e}"))`；Tauri 窗口/WebView2 创建失败由 Tauri 自己 panic（`Failed to setup app: ...`）。
3. 钩子先写 `%TEMP%/hinina-panic.log`（`write_bootstrap_report` 返回是否成功），再按 `logger::ready()` 走 tracing/stderr，**最后**调用 `present_startup_panic`（写入成功才传 `Some(path)`）。
4. 判定通过 → 一次性领取 `PRESENTED` → 独立线程弹 `MessageBoxW` → 等用户关闭 → 返回钩子 → panic 照常 unwind → 进程退出。
5. 事件循环发出 `RunEvent::Ready` → `mark_started()`；此后的 panic 只落日志。

## 测试

`src/infra/tests/startup_dialog_tests.rs`：正文在写入成功时含崩溃路径、失败时明确「未能写入」且不含「崩溃详情：」；判定真值表（启动已完成、未 arm/后台线程都不弹）；一次性领取语义。Windows FFI 与 `console::attach` 同先例不进单测——模态框会阻塞 CI，且正常发布路径无法可靠构造启动失败；本地冒烟用临时 panic 触发，确认进程停在 `MessageBoxW`（自动化会话里窗口不可枚举，未做视觉验收）。

## 设计假设

启动路径没有吞掉 panic 的 `catch_unwind`：tao 0.35 在 Windows 窗口回调处 `catch_unwind` 后会在消息循环里 `resume_unwind`（避免 panic 穿过 Win32 FFI），不是恢复运行；Tauri/wry 没有 `catch_unwind`。因此启动期主线程 panic 都按致命处理。若未来引入真正吞掉 panic 的捕获，必须在捕获点之后调用 `mark_started` 或增加抑制入口。
