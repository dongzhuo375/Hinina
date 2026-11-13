# bootstrap/early

## 职责
Tauri Builder 之前的进程级前置步骤，全部作用于进程全局状态且有严格先后：panic 钩子最先安装 → 启动错误框 arm → `--console` 附加调试控制台 → `--settings` 固化启动标志。**无单测**：钩子安装、控制台附加与 `OnceLock` 固化都是进程级副作用，与 `infra/single_instance` 同属「不为测试制造全局状态」的刻意例外。

## 核心类型/函数
- `fn prepare()` — 按顺序执行：
  1. `infra::panic_report::install()`：此后任何 panic（包括参数解析、恢复、初始化）都有持久落点；先装是纵深防御（历史上的 `env::args()` 会 panic 且 GUI 子系统下毫无输出）。
  2. `infra::startup_dialog::arm()`：启用启动期错误框（测试进程不 arm，绝不弹窗）。
  3. `args_os().skip(1)` 收集参数；`console::requested` → `console::attach`。重绑的 std 句柄必须被 panic 钩子的 stderr 兜底与 `Logger::init` 的 stderr 层接住，因此**必须早于 Logger 与任何 stdio 使用**。
  4. `infra::launch_args::init(&argv)`：`--settings` 管理方钥匙，解析语义沿 `--console`（精确匹配、未知参数忽略），结果经 `launch_flags` 命令交前端。

## 直接依赖
- `crate::infra::{panic_report, startup_dialog, console, launch_args}`
- `std::ffi::OsString` / `std::env::args_os`

## 被依赖
- `src-tauri/src/main.rs`（`fn main` 第一步）

## 逻辑流程
`prepare()` 顺序执行上述四步，无返回值。本模块不创建运行时、不触碰 Tauri，确保任何 panic 都在钩子与错误框就绪之后发生。
