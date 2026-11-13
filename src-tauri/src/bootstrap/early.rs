// Tauri Builder 之前的进程级前置步骤。
//
// 这些步骤都作用于进程全局状态，且有严格先后：panic 钩子必须最先装，控制台重绑
// 必须早于 Logger 与任何 stdio 使用，`--settings` 解析紧随 `--console`。本模块
// 无单测：钩子安装、控制台附加与 OnceLock 固化都是进程级副作用，与
// `infra/single_instance` 同属「不为测试制造全局状态」的刻意例外。

use std::ffi::OsString;

/// 执行 Tauri 创建之前的所有启动步骤。
///
/// 顺序：panic 钩子 → 启动错误框 arm → `--console` 附加 → `--settings` 固化。
pub fn prepare() {
    // panic 钩子最先安装：此后任何 panic（参数解析、恢复、初始化）都有落点。
    // 参数已改用 `args_os()` 收集，本身不会因非 Unicode 参数 panic；钩子先装
    // 是**纵深防御** —— 历史上的 `env::args()` 会 panic 且 GUI 子系统下毫无输出。
    crate::infra::panic_report::install();

    // 启动期对话框：钩子在崩溃记录落盘后弹一次系统错误框，覆盖三处启动
    // expect 与 Tauri 自身创建窗口失败。测试进程不 arm，绝不弹窗。
    crate::infra::startup_dialog::arm();

    // --console：release GUI 构建重新附加调试控制台（dev 构建本就有控制台，
    // 幂等）。必须早于 Logger::init 与任何 stdio 使用：重绑的 std 句柄要被
    // panic 钩子的 stderr 兜底与 Logger::init 的 stderr 层接住，晚于它们就晚了。
    // 参数用 args_os 收集：args() 遇非 Unicode 参数会 panic。
    let argv: Vec<OsString> = std::env::args_os().skip(1).collect();
    if crate::infra::console::requested(&argv) {
        crate::infra::console::attach();
    }

    // --settings：管理方钥匙（初始直落设置页 + 维护者层解锁）。解析结果由
    // launch_flags 命令交给前端；解析语义沿 --console（精确匹配，未知参数忽略）。
    crate::infra::launch_args::init(&argv);
}
