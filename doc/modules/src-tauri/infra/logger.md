# logger

## 职责
日志追踪模块（基于 Tracing）。**双路输出**：stderr（开发调试）+ `{base_dir}/logs/hinina.log` 文件（现场排障 —— 选手机器上没有控制台），支持 debug/release 自适应级别与 RUST_LOG 覆盖。日志文件采用「截断式轮转」防止无限膨胀。

## 核心类型/函数
- 常量 **`pub LOG_RELATIVE_PATH = "logs/hinina.log"`** — 日志文件相对 base_dir 的路径，**落盘与展示的唯一事实来源**：`open_log_file` 用它写日志，`commands/config_cmd.rs` 的 `get_storage_info` 直接引用同一常量拼出展示路径（`base_dir.join(LOG_RELATIVE_PATH)`），杜绝两处硬编码漂移
- 常量 `MAX_LOG_FILE_BYTES = 5MB` — 日志文件大小上限，超过即在启动时截断重开。采用「截断式轮转」而不是按天/按份数滚动：客户端无长期日志留存需求，日志只服务于近期排障，单文件 + 上限截断足以防止占满选手磁盘
- **`Logger`** — 日志 struct，持有日志文件**绝对路径**（`log_path: Option<PathBuf>`，文件层不可用时为 `None`）
- **`Logger::init(base_dir: &Path) -> Self`** — 初始化日志系统（返回实例供运行期清理日志）：
  - debug 构建默认 `debug` 级别，release 默认 `info`，可通过 `RUST_LOG` 环境变量覆盖（同时作用于两路输出）
  - 用 `tracing_subscriber::registry()` + 两个 `fmt::layer()` 组装：stderr 层与文件层均输出 span enter/exit、target、线程 ID、文件名、行号；文件层额外 `with_ansi(false)`（文件里的 ANSI 转义序列不可读，必须关闭着色）
  - **文件层失败（磁盘只读等）时降级为仅 stderr，不阻断启动**
- **`SharedFileWriter`**（私有） — 多线程共享的日志文件写入器，包装 `Arc<Mutex<File>>` 并实现 `io::Write` + `MakeWriter`：tracing 每次事件都会取一个 `io::Write`，而 `File` 不是 `Sync`，故共享句柄并逐条加锁写入；锁中毒时 `into_inner()` 恢复（日志不应因一次 panic 永久失效）
- **`Logger::open_log_file(base_dir) -> Option<File>`**（私有） — 打开日志文件（追加模式）：自动创建 `logs/` 目录；启动时检查大小，**超过 5MB 先截断清空**（rotate-by-truncate，判据是 `>` 而非 `>=`，恰好等于上限不截断）；任何一步失败返回 `None` 由调用方降级
- **`Logger::clear_log_file(&self) -> AppResult<u64>`** — 清空日志内容并返回释放的字节数（文件层不可用时为 0）。**运行期**清理入口，供设置页「清理本地数据」调用
- **`Logger::has_log_file(&self) -> bool`** — 文件层是否可用。用于区分「清空了日志」与「本来就没有文件层（磁盘只读等）」，让界面如实说明结果
- **`truncate_log_file(path) -> io::Result<u64>`**（私有自由函数） — 把日志文件截断为 0 并返回释放字节数。抽成自由函数是为了可单测（`init` 会设置全局 subscriber，测试里不能调）

## 直接依赖
- `std::fs` / `std::io` / `std::path::Path` / `std::sync::{Arc, Mutex}`
- `tracing_subscriber::{fmt, fmt::MakeWriter, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter}`（Cargo.toml 中启用 `fmt` / `registry` features）

## 被依赖
- `core::context`（`AppContext::init()` 序列第一步调用 `Logger::init(&base_dir)` 并**持有返回的实例**（`AppContext.logger`），后续步骤才能记录日志）
- `commands::maintenance_cmd`（`purge_local_data` → `clear_log_file` / `has_log_file`）
- `commands::config_cmd`（`get_storage_info` 引用 `LOG_RELATIVE_PATH` 拼出日志展示路径）

## 逻辑流程
`Logger::init(base_dir)` 在应用启动时最先调用：构造 `EnvFilter`（RUST_LOG 优先，否则按编译模式取默认级别）→ 构造 stderr 层 → `open_log_file(base_dir)` 尝试打开文件层（建目录 → 超限截断 → 追加打开）→ 成功则 `registry + filter + stderr 层 + 文件层` 三层组装 `init()`；失败则 `eprintln!` 留痕并退回单路 stderr。此后所有 `tracing` 宏事件同时写往两路。

## 测试
`src-tauri/src/infra/tests/logger_tests.rs`（由 `logger.rs` 底部 `#[cfg(test)] #[path = "tests/logger_tests.rs"] mod tests;` 引用）只测纯文件行为，**不测 `init`**（全局 subscriber 每进程只能设置一次，在测试运行时里 init 会与 cargo test 的输出捕获及其他测试冲突）。用例（各自建独立临时目录，用后清理）：自动创建 `logs/hinina.log`、未超限的既有日志追加而不清空、超过 5MB 的文件启动时截断为 0、恰好等于上限不截断（锁定 `>` 判据边界）。

**运行期清理**：`truncate_log_file` 在**追加句柄仍打开**时也能清空（这正是不能用 `set_len` 的约束 —— 本用例必须先打开句柄再清理，否则测不到它）、释放字节数如实回报、清空后后续写入从 0 开始（不写出稀疏文件）、空文件清理返回 0。

## 设计要点

- **运行期清理用「重开 + 截断」而不是句柄 `set_len`**：日志句柄是**追加模式**打开的（只有 `FILE_APPEND_DATA`，没有 `FILE_WRITE_DATA`），Windows 上 `set_len` 会被拒（实测 `Os code 5, PermissionDenied`）。新句柄带 `GENERIC_WRITE`，而两个句柄的共享模式都允许对方写入，故互不冲突；`append` 模式保证后续日志自动从 0 开始。这与启动时的截断式轮转是同一手法（`fs::write(path, [])`），两处不会漂移。
- **只清内容、不删文件**：删了要等下次启动才重建，中间这段排障信息就彻底没了。
