# single_instance

## 职责

单实例约束：同一次只允许运行一个应用实例。基于 `tauri-plugin-single-instance`（官方插件，Rust 侧薄封装在 `init()`）：Windows 用命名互斥体、Linux 用 DBus、macOS 用本地 socket，句柄/服务随进程死亡自动释放，无残留锁。第二实例在 **Tauri Builder.build() 的插件初始化阶段**即被识别并退出——早于 `.setup()`：不建 AppContext、不解析数据目录、不开日志、不触发启动失败框。被拦截时把本次启动（args/cwd）通知第一实例，第一实例回调聚焦已有主窗口，让「已经开着了」可见。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `init` | `pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry>` | 注册插件。**必须第一个注册**（官方要求：检测须早于其它插件初始化，main.rs Builder 链首位）。回调忽略 args/cwd（无深链/文件关联；未来引入时在此转发给第一实例） |
| `focus_main_window` | `fn focus_main_window(app: &tauri::AppHandle)`（私有） | 回调实体：按 label `main` 找窗口（tauri.conf.json 未配置 label，Tauri 默认 `main`），不存在（首实例尚在启动/已在退出）早退；最小化先 `unminimize` 再 `set_focus`。**所有窗口操作结果一律忽略**——回调跑在第一实例，panic 会经 panic 钩子写崩溃文件并弹启动失败框，把无害的重复双击变成致命故障，绝不 expect/unwrap |

## 直接依赖

- `tauri-plugin-single-instance =2.4.5`（**精确锁定，非 caret**——2.5.x 起要求 tauri ^2.12，caret 下任何 `cargo update` 都会静默连带升级 tauri/tao/wry 整个运行栈，等下次 tauri 升级一起抬；版本策略详见 Cargo.toml 注释。3.0.0-alpha 是 Tauri v3 线不适用。**纯 Rust 侧插件**：无 JS API、无 npm 包、无 capability 声明，不涉 dialog 那条的「npm 包与 Rust crate 对齐」要求）
- `tauri::Manager`（`get_webview_window`；句柄操作 `is_minimized` / `unminimize` / `set_focus`）

## 被依赖

- `main.rs`（Builder 链首位 `.plugin(hinina_lib::infra::single_instance::init())`，先于 `tauri_plugin_dialog`）

## 逻辑流程

1. `main()` 装 panic 钩子 → `arm()` → `--console` 解析/附加 → Tokio runtime → Builder 链。
2. **第一实例**：插件初始化创建命名互斥体成功 → 正常继续（setup、窗口、事件循环）。
3. **第二实例**：插件初始化发现互斥体已被持有 → 经 IPC 通知第一实例（args/cwd）→ 插件内直接退出进程，不进 setup。边角：若带 `--console`，此时已先附加控制台（console attach 早于 Builder），控制台窗口闪一下再退出，属预期。
4. **第一实例回调**：`focus_main_window`——窗口不存在早退；最小化先还原；`set_focus`。任一步失败静默保持现状。

## 测试

无单测。曾尝试用 tauri mock runtime（dev-dependency `tauri` 的 `test` feature）测 `focus_main_window` 兜底逻辑，实测链入后本机 lib 测试二进制**启动即崩**（`STATUS_ENTRYPOINT_NOT_FOUND` 0xc0000139；二分定位到 mock 测试代码本身——坏二进制额外链入整套真实 wry/tao 窗口导入 user32/gdi32/comctl32/dwmapi/shell32/ole32，移除测试后 805 用例全绿）。为不污染日常 `cargo test`，不留单测。行为验证 = 双开脚本核进程数 + 人工验收聚焦效果；回调的无 panic 约束靠代码评审把关（`init` 只有一次函数调用，`focus_main_window` 无 unwrap/expect）。

## 设计假设

- **互斥体身份**：插件按应用 identifier（`com.hinina.app`）命名互斥体/DBus 服务。dev 与 release 共享同一 identifier——一方在跑时启动另一方同样被拦截并聚焦前者，属预期。
- **无跨进程转发需求**：本应用无深链/文件关联，第二实例的 args/cwd 直接丢弃；未来若引入（`hinina://` 协议或「用 Hinina 打开」），在回调里把 args 转发给第一实例，而不是新开实例。
- **进程崩溃不残留锁**：Windows 互斥体由内核在持有进程死亡时释放；Linux DBus 服务名随总线注销。不存在「上次没退干净导致再也打不开」的失败模式，无需 stale-lock 清理。
- **与 panic 钩子的顺序无耦合**：第二实例在 panic 钩子安装**之后**、Builder 之前退出——钩子已装但不会触发（无 panic 路径）；`arm()`/`mark_started()` 的语义不受影响。
