//! 单实例约束：同一次只允许运行一个应用实例。
//!
//! 基于 `tauri-plugin-single-instance`（官方插件；Windows 走命名互斥体、
//! Linux 走 DBus、macOS 走本地 socket，句柄随进程死亡由内核释放，无残留
//! 锁）。检测发生在 **Builder.build() 的插件初始化阶段**：第二实例发现
//! 互斥体已被持有，把本次启动（args/cwd）通知第一实例后立即退出 ——
//! **早于 `.setup()`**：不建 AppContext、不解析数据目录、不开日志、不触发
//! 启动失败框。已知边角：第二实例若带 `--console`，会先附加控制台再退出
//!（main() 里 console attach 早于 Builder），闪一下属预期。
//!
//! **回调跑在第一实例**（收到第二实例启动通知时触发），聚焦已有主窗口，
//! 让「已经开着了」可见。回调里**绝不 panic**：panic 会经 panic 钩子写
//! 崩溃文件并弹启动失败框，把一次无害的重复双击变成致命故障 —— 所有窗口
//! 操作的结果一律忽略（聚焦失败 = 保持现状，无副作用可回收）。
//!
//! dev 与 release 构建共享同一 identifier（`com.hinina.app`）：一方在跑时
//! 启动另一方同样被拦截并聚焦前者。互斥体按 identifier 命名，属预期。
//!
//! **无单测**：曾尝试用 tauri mock runtime 测本模块兜底逻辑，实测会把
//! 本机 lib 测试二进制变成启动即崩（`0xc0000139`，坏二进制额外链入整套
//! 真实 wry/tao 窗口栈，取证见 PR 描述）——为不污染日常 `cargo test`，
//! 兜底逻辑不留单测；行为验证 = 双开脚本核进程数 + 人工验收聚焦效果。

use tauri::Manager;

/// 主窗口 label（tauri.conf.json 未显式配置，Tauri 默认 `"main"`）。
const MAIN_WINDOW_LABEL: &str = "main";

/// 注册单实例插件。**必须第一个注册**（先于其它插件；官方要求检测尽早
/// 发生，其它插件不得抢先初始化）。
pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_single_instance::init(|app, _args, _cwd| {
        focus_main_window(app);
    })
}

/// 聚焦（必要时还原）第一实例的主窗口。窗口不存在时静默返回：第一实例
/// 尚在启动（窗口未创建）或已进入退出流程，第二实例反正已被拦截退出。
fn focus_main_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return;
    };
    if window.is_minimized().unwrap_or(false) {
        let _ = window.unminimize();
    }
    let _ = window.set_focus();
}
