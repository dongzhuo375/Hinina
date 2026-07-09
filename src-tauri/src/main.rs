// Hinina 入口点
//
// 初始化顺序：
//   1. Logger          — 最先初始化，后续步骤可记录日志
//   2. ConfigService   — 加载配置，决定后续行为
//   3. Storage         — 文件系统根目录
//   4. HttpClient      — Reqwest 客户端
//   5. EventBus        — 事件总线（纯内存，可较早初始化）
//   6. ProviderRegistry — 注册各 OJ Adapter
//   7. WorkspaceManager — 扫描并恢复工作区
//   8. AppContext      — 装配上述所有
//   9. Tauri App       — 注入 AppContext 到 State

use hinina_lib::commands;
use hinina_lib::core::context::AppContext;

fn main() {
    // 运行时初始化，阻塞式
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let ctx = rt.block_on(async {
        // TODO: 从 Tauri app_data_dir 获取正式路径（阶段 2 实现后完善）
        let base_dir = std::env::temp_dir().join("hinina");
        AppContext::init(base_dir)
            .await
            .expect("Failed to initialize AppContext")
    });

    tauri::Builder::default()
        .manage(ctx)
        .setup(|app| {
            commands::register_commands(app);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Failed to launch Hinina");
}
