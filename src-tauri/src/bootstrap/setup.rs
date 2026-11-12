// `.setup()` 内的应用装配。
//
// 数据目录解析与一次性迁移必须早于 `AppContext::init`：init 会打开日志文件，
// 之后旧目录里的 logs/ 就被占住（虽然迁移清单不含 logs，但把顺序固定下来能让
// 「谁在什么时候动文件」保持可推理）。
//
// 事件消费者在 `manage(AppContext)` 之后启动：它们从 `app.state::<AppContext>()`
// 取总线与插件宿主，只观察事实、不参与任何业务动作。

use std::io::Write as _;
use std::sync::Arc;

use tauri::Manager;

use crate::bootstrap::events;
use crate::bootstrap::notification::TauriPluginNotifications;
use crate::core::context::AppContext;
use crate::infra::data_dir::{self, DataDirPlan, DataDirSource, MigrateOutcome};

/// 在 Tauri setup 阶段装配 `AppContext` 与全部运行时组件。
pub fn install(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // 运行时只在 setup 期间服务 `AppContext::init` 的 block_on：setup 是同步
    // 回调，而 init 是 async。**事件消费者不跑在这个运行时上**（它随 setup
    // 结束一起销毁），而是由 `events::spawn_event_consumers` 投到 Tauri 的
    // 运行时 —— 与异步命令同一个。
    let rt = tokio::runtime::Runtime::new()
        .unwrap_or_else(|error| panic!("无法创建后台运行时：{error}"));

    // AppContext 必须在这里初始化而不是 main()：只有 setup 能拿到 AppHandle，
    // 也就只有这里能解析 `app_local_data_dir()`。
    //
    // 解析失败**不静默顶替**：把原因交给 prepare_startup 显式降级为
    // FallbackTemp（临时目录可用也不会被误报成 Default，强告警不丢）。
    // release 是 GUI 子系统、stderr 句柄无效：写失败必须忽略
    // （`eprintln!` 在写失败时会 panic，让启动直接崩掉）。
    let legacy = data_dir::legacy_dir();
    let (default_dir, default_dir_error) = match app.path().app_local_data_dir() {
        Ok(dir) => (dir, None),
        Err(e) => {
            let _ = writeln!(
                std::io::stderr(),
                "解析 app_local_data_dir 失败，回退临时目录: {e}"
            );
            (
                legacy.clone(),
                Some(format!("app_local_data_dir 解析失败: {e}")),
            )
        }
    };

    // ① 一次性数据迁移必须在 `AppContext::init` **之前**：init 会打开日志文件，
    //    之后旧目录里的 logs/ 就被占住（虽然迁移清单不含 logs，但把顺序固定下来
    //    能让「谁在什么时候动文件」保持可推理）。
    let (plan, migration) = data_dir::prepare_startup_with_default_error(
        default_dir_error.as_deref(),
        &default_dir,
        &legacy,
    );

    let ctx = rt
        .block_on(AppContext::init(
            plan.base_dir.clone(),
            plan.default_dir.clone(),
            plan.source,
        ))
        .unwrap_or_else(|error| {
            // init 失败仍走 panic（钩子落盘 + 启动错误框语义不变），但**先**
            // 汇报数据目录诊断：Logger::init 是 init 的第 1 步且不失败，走到
            // 这里订阅器基本已就绪，汇报能落日志——失败路径不能把「为什么」
            // 一起带走（若未来 init 在 Logger 之前就可能失败，此调用落不到
            // 任何输出，无害）
            report_data_dir(&plan, &migration);
            panic!("无法初始化应用数据：{}", error.user_message())
        });

    // ② 迁移结果与数据目录来源必须**在日志就绪后**立刻汇报
    report_data_dir(&plan, &migration);

    ctx.plugin_manager
        .set_output(Arc::new(TauriPluginNotifications(app.clone())));
    let plugins = ctx.plugin_manager.clone();
    app.manage(ctx);
    // ③ 消费者在 AppContext 就绪后启动：它们只观察事实，不参与任何业务动作
    events::spawn_event_consumers(app);
    tauri::async_runtime::spawn(async move {
        if let Err(error) = plugins.autostart().await {
            // 与 host.start 同一形状：插件域宿主级操作，无具体插件可归属
            // （plugin_id/plugin_version 留空）；error 已是白名单化的 PluginError
            tracing::warn!(plugin_id = "", plugin_version = "", runtime = "js", event = "",
                operation = "host.autostart", permission = "", request_id = "", error = ?error,
                "插件配置不可用，自动加载已跳过");
        }
    });
    Ok(())
}

/// 汇报数据目录方案与迁移结果（在日志就绪后调用）。
///
/// 四件事必须说清楚，因为它们都影响「我的数据到底在哪、会不会丢」：
/// - **订阅器就绪前的诊断**（指针解析失败、候选目录因何不可用、迁移条目失败等）
///   → `replay` 重放（`prepare_startup` 已把迁移诊断按发生顺序并入
///   `plan.warnings`），这是排障者回答「为什么我的自定义目录没生效」的唯一线索；
/// - **回退到临时目录** → `error` 级告警（数据随时可能被系统清理，这是本次修复要消除的状态）；
/// - **迁移有失败项** → `error` 级告警并列出条目（下次启动会自动重试）；
/// - **迁移成功** → `info` 记录去向。
fn report_data_dir(plan: &DataDirPlan, migration: &MigrateOutcome) {
    data_dir::replay(&plan.warnings);
    match plan.source {
        DataDirSource::Default => {
            tracing::info!(dir = %plan.base_dir.display(), "数据目录（默认）");
        }
        DataDirSource::Custom => {
            tracing::info!(dir = %plan.base_dir.display(), "数据目录（用户指定）");
        }
        DataDirSource::FallbackTemp => {
            tracing::error!(
                dir = %plan.base_dir.display(),
                default_dir = %plan.default_dir.display(),
                "数据目录回退到临时目录：数据随时可能被系统清理！请在设置页更改数据目录"
            );
        }
    }

    if migration.is_noop() {
        return;
    }
    if migration.is_ok() {
        tracing::info!(
            moved = ?migration.moved,
            skipped = ?migration.skipped,
            legacy_removed = migration.legacy_removed,
            "数据已迁移到新的数据目录"
        );
    } else {
        // 不阻断启动：数据仍在原处，客户端能正常工作；但必须让用户与排障者看见
        tracing::error!(
            moved = ?migration.moved,
            failed = ?migration.failed,
            "数据迁移未全部完成（下次启动会自动重试）；失败项仍留在原目录"
        );
    }
}
