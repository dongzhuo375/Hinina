// 数据目录 Command（设置页「数据目录」）。
//
// 三条命令对应界面上的三件事：看当前用哪个目录、改到别处、恢复默认。
//
// **改动一律「下次启动生效」**：各 Service 都持有以 base_dir 为根的 `Storage`，
// 日志还握着文件句柄 —— 运行中热切等于重建整个 `AppContext`，风险极高且收益为零
// （选手改数据目录是低频动作）。故这里只做「校验 + 写位置指针」，真正的搬运由
// 下次启动在 `Logger::init` 之前完成（见 `infra::data_dir` 的模块头注释）。
//
// 也正因如此，**不在运行中迁移**：那样会让「已迁移的旧目录」与「仍在写入的旧目录」
// 产生分叉，重启后这段写入就丢了。改目录时只把「待迁移来源」写进指针。

use serde::Serialize;
use tauri::State;
use tracing::info;

use crate::core::context::AppContext;
use crate::core::error::{AppError, AppResult};
use crate::infra::data_dir::{self, DataDirPointer, DataDirSource};

/// 当前数据目录信息（供设置页如实展示「在用哪个目录、为什么、是否需要重启」）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataDirInfo {
    /// 当前**生效**的数据目录（`Storage` 的根）
    pub current_dir: String,
    /// 默认数据目录（`app_local_data_dir()`，「恢复默认」的目标）
    pub default_dir: String,
    /// 来源：`default` / `custom` / `fallbackTemp`
    pub source: DataDirSource,
    /// 是否有改动待重启生效（指针指向的目录 ≠ 当前生效目录，或有待迁移来源）
    pub restart_required: bool,
}

/// 更改结果（供界面提示「已记录，重启后生效」）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataDirChange {
    /// 重启后将使用的数据目录
    pub target_dir: String,
    /// 重启时将从这个目录搬运数据（`None` = 不迁移）
    pub migrate_from: Option<String>,
    /// 恒为 `true`：数据目录改动只能重启生效（见文件头注释）
    pub restart_required: bool,
}

/// 读取当前数据目录信息。
///
/// 前端 invoke 签名: `get_data_dir`
#[tauri::command]
pub async fn get_data_dir(ctx: State<'_, AppContext>) -> AppResult<DataDirInfo> {
    let current = ctx.storage.base_dir().to_string_lossy().into_owned();
    let pointer = data_dir::read_pointer(&ctx.default_data_dir);

    // 待重启的两种情形：指针已指向别处，或还有没搬完的数据
    let restart_required = pointer.migrate_from.is_some()
        || pointer
            .data_dir
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .is_some_and(|s| s != current);

    Ok(DataDirInfo {
        current_dir: current,
        default_dir: ctx.default_data_dir.to_string_lossy().into_owned(),
        source: ctx.data_dir_source,
        restart_required,
    })
}

/// 把数据目录改到指定路径。
///
/// 前端 invoke 签名: `set_data_dir`({ path, migrate })
///
/// - 校验目标目录（绝对路径 / 非文件 / **必须为空** / 非临时目录 / 可写）；
/// - 写入位置指针；`migrate=true` 时同时记下「待迁移来源 = 当前目录」；
/// - **不在运行中搬运**（见文件头注释），改动重启后生效。
#[tauri::command]
pub async fn set_data_dir(
    ctx: State<'_, AppContext>,
    path: String,
    migrate: bool,
) -> AppResult<DataDirChange> {
    let target = data_dir::validate_target(
        std::path::Path::new(path.trim()),
        &data_dir::legacy_dir(),
    )?;
    let current = ctx.storage.base_dir().to_path_buf();
    if target == current {
        return Err(AppError::Config("目标目录与当前数据目录相同".into()));
    }

    info!(target = %target.display(), migrate, "记录新的数据目录（重启后生效）");
    write_target(&ctx, Some(target.clone()), migrate.then(|| current.clone()))?;

    Ok(DataDirChange {
        target_dir: target.to_string_lossy().into_owned(),
        migrate_from: migrate.then(|| current.to_string_lossy().into_owned()),
        restart_required: true,
    })
}

/// 恢复默认数据目录。
///
/// 前端 invoke 签名: `reset_data_dir`({ migrate })
///
/// 与 [`set_data_dir`] 的区别是**不做「必须为空」校验**：默认目录是客户端自己的地盘，
/// 可能残留上次用过的数据（用户之前改走过）。此时迁移会跳过已存在的条目而不是覆盖，
/// 因此「部分合并」是安全的。
#[tauri::command]
pub async fn reset_data_dir(
    ctx: State<'_, AppContext>,
    migrate: bool,
) -> AppResult<DataDirChange> {
    let default_dir = ctx.default_data_dir.clone();
    if !data_dir::dir_is_usable(&default_dir) {
        return Err(AppError::Config(format!(
            "默认数据目录不可用: {}",
            default_dir.display()
        )));
    }
    let current = ctx.storage.base_dir().to_path_buf();
    if default_dir == current {
        return Err(AppError::Config("当前已在默认数据目录".into()));
    }

    info!(target = %default_dir.display(), migrate, "恢复默认数据目录（重启后生效）");
    write_target(&ctx, None, migrate.then(|| current.clone()))?;

    Ok(DataDirChange {
        target_dir: default_dir.to_string_lossy().into_owned(),
        migrate_from: migrate.then(|| current.to_string_lossy().into_owned()),
        restart_required: true,
    })
}

/// 弹出原生目录选择器，返回用户选中的路径（取消则 `None`）。
///
/// 前端 invoke 签名: `pick_data_dir`
///
/// 用**回调 + oneshot** 而不是 `blocking_pick_folder`：后者会阻塞当前线程，
/// 而命令跑在异步运行时上（阻塞工作线程是浪费，某些平台还要求弹窗在主线程）。
#[tauri::command]
pub async fn pick_data_dir(app: tauri::AppHandle) -> AppResult<Option<String>> {
    use tauri_plugin_dialog::DialogExt;

    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |picked| {
        // 接收端可能已消失（用户重复点击导致命令被取消），忽略发送失败
        let _ = tx.send(picked);
    });

    let picked = rx
        .await
        .map_err(|e| AppError::Unknown(format!("目录选择器无响应: {}", e)))?;

    Ok(picked
        .and_then(|file_path| file_path.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned()))
}

/// 写入位置指针：`data_dir=None` 表示用默认目录。
fn write_target(
    ctx: &State<'_, AppContext>,
    data_dir: Option<std::path::PathBuf>,
    migrate_from: Option<std::path::PathBuf>,
) -> AppResult<()> {
    let pointer = DataDirPointer {
        data_dir: data_dir.map(|p| p.to_string_lossy().into_owned()),
        migrate_from: migrate_from.map(|p| p.to_string_lossy().into_owned()),
        // 设置页改目录不涉及「临时目录一次性搬家」标记，保持原值（由 read-modify-write 补）
        legacy_migrated: data_dir::read_pointer(&ctx.default_data_dir).legacy_migrated,
    };
    data_dir::write_pointer(&ctx.default_data_dir, &pointer)
}
