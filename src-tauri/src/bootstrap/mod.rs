// Hinina 组合根：在 `main.rs` 与各子系统之间做装配。
//
// 为什么整个初始化在 `.setup()` 里而不是 `main()`：只有 setup 能拿到 AppHandle，
// 也就只有那里能解析 `app_local_data_dir()`（数据根目录）。
//
// 主侧启动顺序（`AppContext::init` 的 1–12 步以 `core/context.rs` 为唯一编号来源，
// 这里不复制）：
//   前置（Tauri Builder 之前）— `early::prepare`：
//     panic 钩子 → 启动错误框 arm → `--console` 附加 → `--settings` 固化
//   setup 内 — `setup::install`：
//     ① 数据目录：解析 `app_local_data_dir()` + 一次性迁移（**必须最先**，日志一旦
//        打开就会占住旧目录）
//     ② `AppContext::init`（1–12 步，见 core/context.rs）
//     ③ 日志就绪后重放数据目录诊断
//     ④ 接上插件通知出口并 `manage(AppContext)`
//     ⑤ 启动事件消费者（投到 Tauri 运行时；不得用 setup 内的局部 runtime）
//     ⑥ 插件 autostart
//   收尾：`commands::invoke_handler` 注册全部 Command → build → run
//     （`Ready` 标记启动完成；`Exit` 收尾插件）

pub mod early;
pub mod events;
pub mod notification;
pub mod setup;

use tauri::Manager;

use crate::core::context::AppContext;
use crate::infra;

/// 构造 Tauri Builder 并注册三个内建插件。
///
/// 单实例约束：同一次只允许运行一个实例，重复启动退出并聚焦已有窗口。
/// 官方要求**第一个注册**（检测须早于其它插件初始化）；第二实例在此
/// 阶段即退出，不进 setup，见 infra/single_instance。
///
/// 目录选择器（设置页「数据目录」）。本项目不注册 tauri-plugin-fs / -http
/// （fs/http 的依赖与 capability 存在但从未注册，全部 I/O 都走 Rust）。
///
/// 外链转交系统浏览器（app_cmd::open_external 经 Rust API 消费）。Rust-only
/// 注册：不装 npm 包、不声明 opener capability——插件的 IPC 命令虽已在
/// invoke handler 注册，但 webview 调用一律被 ACL 拒绝（无授权即拒绝）。
/// **关闭默认开启的 `open_js_links_on_click`**：默认值 true 会给 webview 注入
/// 点击脚本，对 _blank / Ctrl / Shift 点击的 http(s)/mailto/tel 锚点直接调
/// `plugin:opener|open_url`——该 invoke 必经 ACL 拒绝并成为 unhandled
/// rejection（每次点击一条错误日志），且一旦将来有人补了 capability 就会
/// 成为绕过 open_external 白名单的第二条转交通道。关掉才保证唯一入口。
/// scheme 白名单由该命令守卫（见 app_cmd.rs）。
pub fn builder() -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default()
        .plugin(infra::single_instance::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_opener::Builder::default()
                .open_js_links_on_click(false)
                .build(),
        )
}

/// 运行期事件：启动完成标志与退出收尾。
pub fn handle_run_event(app: &tauri::AppHandle, event: tauri::RunEvent) {
    match event {
        tauri::RunEvent::Ready => {
            // Tauri 的 setup 先按配置创建窗口（WebView2 在此初始化），再调用
            // 用户的 setup 闭包；Ready 在 setup 返回后才发出（tauri app.rs 的
            // `fn setup` / `make_run_event_loop_callback`）。因此窗口/WebView2
            // 创建失败时本标志尚未置位，启动错误框仍会弹出。
            infra::startup_dialog::mark_started();
        }
        tauri::RunEvent::Exit => {
            if let Some(ctx) = app.try_state::<AppContext>() {
                ctx.plugin_manager.cancel_all();
            }
        }
        _ => {}
    }
}
