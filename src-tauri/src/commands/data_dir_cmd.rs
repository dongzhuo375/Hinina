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
    /// 位置指针是否落在临时目录（`app_local_data_dir()` 解析失败的降级态）。
    ///
    /// 与 `source` 正交：降级 + 已保存的自定义目录生效时 `source == custom`，
    /// 但指针仍在临时目录，临时目录被清理后自定义目录设置会丢失。界面据此
    /// **持续**告警，而不是只在用户改目录的那一刻提示。
    pub pointer_in_temp: bool,
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
    /// 附加风险提示（`Some` 时界面必须替换成功文案展示）。
    ///
    /// 目前唯一的来源是**降级模式**（`default_data_dir == %TEMP%/hinina`，即
    /// `app_local_data_dir()` 解析失败、指针也落在临时目录）：系统清理临时文件后
    /// 本次自定义目录设置会一起丢失。不提示就等于「用户以为已经换到安全位置，
    /// 实际只改了一个随时会消失的标记」。注意判据不是 `source`：降级 + 已保存的
    /// 自定义目录生效时 `source == Custom`，但指针仍在临时目录。
    pub warning: Option<String>,
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
        pointer_in_temp: pointer_in_temp(&ctx.default_data_dir),
    })
}

/// 把数据目录改到指定路径。
///
/// 前端 invoke 签名: `set_data_dir`({ path, migrate })
///
/// - 校验目标目录（`validate_target`：绝对路径 / 非文件 / 非临时目录 / 可写，
///   **`migrate` 时还要求无冲突条目**）；
/// - 写入位置指针；`migrate=true` 时同时记下「待迁移来源 = 当前目录」；
/// - **不在运行中搬运**（见文件头注释），改动重启后生效。
///
/// **不勾迁移时可以改回任何用过的目录**：没有迁移就没有跳过，不存在「静默用旧数据」
/// 的问题 —— 用户是在明确选择「用那个目录里原来的数据」。
#[tauri::command]
pub async fn set_data_dir(
    ctx: State<'_, AppContext>,
    path: String,
    migrate: bool,
) -> AppResult<DataDirChange> {
    let target = data_dir::validate_target(
        std::path::Path::new(path.trim()),
        &data_dir::legacy_dir(),
        migrate,
    )?;
    let current = ctx.storage.base_dir().to_path_buf();
    if target == current {
        return Err(AppError::Config("目标目录与当前数据目录相同".into()));
    }

    info!(target = %target.display(), migrate, "记录新的数据目录（重启后生效）");
    write_target(&ctx, Some(target.clone()), migrate.then(|| current.clone()))?;

    // 降级模式下指针本身落在临时目录：临时目录被清理后本次设置会丢失。
    // 当前界面已有「数据在临时目录」的告警，但**不覆盖「新设置也会丢」**这一层。
    let warning = degraded_pointer_warning(&ctx.default_data_dir);

    Ok(DataDirChange {
        target_dir: target.to_string_lossy().into_owned(),
        migrate_from: migrate.then(|| current.to_string_lossy().into_owned()),
        restart_required: true,
        warning,
    })
}

/// 「恢复默认数据目录」的校验决策（纯函数，便于单测）。
///
/// 抽出来是因为命令本体依赖 `tauri::State`（无法单测），而**校验判据正是 HIGH 缺陷
/// 漏网的地方** —— 用 `dir_is_empty` 时真机上永远失败。把决策变成可测纯函数，
/// 才有人能锁住它。
pub(crate) fn validate_reset(
    default_dir: &std::path::Path,
    current: &std::path::Path,
    migrate: bool,
) -> AppResult<()> {
    if !data_dir::dir_is_usable(default_dir) {
        return Err(AppError::Config(format!(
            "默认数据目录不可用: {}",
            default_dir.display()
        )));
    }
    if default_dir == current {
        return Err(AppError::Config("当前已在默认数据目录".into()));
    }
    if migrate {
        // **判据是「冲突条目」而不是「目录为空」**：默认目录按设计必然含
        // `data_dir.json`（指针固定存于此）与每次启动重建的 `EBWebView/`，
        // 用「空目录」当判据会让本命令在真机上永远失败（且指引是死循环）。
        let conflicts = data_dir::conflicting_entries_in(default_dir);
        if !conflicts.is_empty() {
            return Err(AppError::Config(format!(
                "默认数据目录已有 {}（{}）—— 迁移会跳过它们，界面将回退到那里的陈旧数据。\
                 请先备份并删除这些条目，或不勾选「迁移现有数据」",
                conflicts.join(" / "),
                default_dir.display()
            )));
        }
    }
    Ok(())
}

/// 恢复默认数据目录。
///
/// 前端 invoke 签名: `reset_data_dir`({ migrate })
///
/// **勾了迁移时，若默认目录已有「冲突条目」则拒绝**（[`validate_reset`]）：迁移对
/// 「目标已存在」的条目是**跳过**而非覆盖，于是恢复默认会**静默回退到那些陈旧数据**
/// （当前目录里的新数据被无视）—— 这比报错糟糕得多。
///
/// **判据刻意不是「目录为空」**：默认目录按设计必然含 `data_dir.json`（位置指针固定存于
/// 此）与每次启动重建的 `EBWebView/`，用「空目录」当判据会让本命令**在真机上永远失败**，
/// 且给出的「清空该目录」指引是死循环（`EBWebView` 运行中被锁、删指针则按钮消失）。
/// 只校验**迁移真正会跳过的那些条目**，恰好就是冲突集合。
///
/// 不勾迁移时无需校验：用户只是想切回默认目录，里面有什么就是什么（那是他自己的选择）。
#[tauri::command]
pub async fn reset_data_dir(ctx: State<'_, AppContext>, migrate: bool) -> AppResult<DataDirChange> {
    let default_dir = ctx.default_data_dir.clone();
    let current = ctx.storage.base_dir().to_path_buf();
    validate_reset(&default_dir, &current, migrate)?;

    info!(target = %default_dir.display(), migrate, "恢复默认数据目录（重启后生效）");
    write_target(&ctx, None, migrate.then(|| current.clone()))?;

    Ok(DataDirChange {
        target_dir: default_dir.to_string_lossy().into_owned(),
        migrate_from: migrate.then(|| current.to_string_lossy().into_owned()),
        restart_required: true,
        // 恢复默认的目标就是默认目录本身；降级态下 default==current 已被
        // validate_reset 拒绝，这里不存在「指针仍落临时目录」的新增风险
        warning: None,
    })
}

/// 弹出原生目录选择器，返回用户选中的路径（取消则 `None`）。
///
/// 前端 invoke 签名: `pick_data_dir`
///
/// 用**回调 + oneshot** 而不是 `blocking_pick_folder`：后者会阻塞当前线程，
/// 而命令跑在异步运行时上（阻塞工作线程是浪费，某些平台还要求弹窗在主线程）。
///
/// **先 `try_state` 探测插件**：`DialogExt::dialog()` 内部是 `state::<Dialog<R>>()`，
/// 插件未注册时**直接 panic** —— 那会把「点一下更改目录」变成崩溃。
/// 用 `try_state` 换成可读的错误，代价是一次哈希查找。
#[tauri::command]
pub async fn pick_data_dir(app: tauri::AppHandle) -> AppResult<Option<String>> {
    use tauri::Manager;
    use tauri_plugin_dialog::DialogExt;

    if app
        .try_state::<tauri_plugin_dialog::Dialog<tauri::Wry>>()
        .is_none()
    {
        return Err(AppError::Unknown(
            "目录选择器插件未注册（缺少 .plugin(tauri_plugin_dialog::init())）".into(),
        ));
    }

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

/// 位置指针是否落在临时目录（`app_local_data_dir()` 解析失败的降级态）。
///
/// 判据看 `default_data_dir`（指针所在处）而不是 `data_dir_source`：降级 +
/// 已保存的自定义目录生效时 `source == Custom`，但默认目录仍是临时目录，
/// 指针依旧会随临时目录清理消失。
pub(crate) fn pointer_in_temp(default_data_dir: &std::path::Path) -> bool {
    default_data_dir == data_dir::legacy_dir()
}

/// 降级模式下指针所在位置的附加风险提示（纯函数，便于单测）。
pub(crate) fn degraded_pointer_warning(default_data_dir: &std::path::Path) -> Option<String> {
    pointer_in_temp(default_data_dir).then(|| {
        "当前处于临时目录降级模式：位置指针也保存在临时目录，系统清理临时文件后本次更改会丢失；\
         请在重启后确认当前目录，并尽快让默认数据目录恢复可用"
            .to_string()
    })
}

/// 写入位置指针：`data_dir=None` 表示用默认目录。
///
/// `legacy_migrated` 必须从磁盘**严格**读回（只有 NotFound 视为 None）：用宽松的
/// `read_pointer` 会把损坏/不可读的指针当成默认值，写回时抹掉 `Some(true)`，
/// 下次启动重新执行一次性搬家、把用户已删除的旧数据拷回来（评审）。读不出来时
/// 宁可拒绝本次更改并让用户修复指针文件。
fn write_target(
    ctx: &State<'_, AppContext>,
    data_dir: Option<std::path::PathBuf>,
    migrate_from: Option<std::path::PathBuf>,
) -> AppResult<()> {
    let existing = data_dir::read_pointer_strict(&ctx.default_data_dir).map_err(|e| {
        AppError::Config(format!(
            "现有数据目录指针不可读或已损坏，已拒绝更新（避免丢失迁移标记；可修复或删除 data_dir.json 后重试）: {}",
            e.user_message()
        ))
    })?;
    let pointer = DataDirPointer {
        data_dir: data_dir.map(|p| p.to_string_lossy().into_owned()),
        migrate_from: migrate_from.map(|p| p.to_string_lossy().into_owned()),
        // 设置页改目录不涉及「临时目录一次性搬家」标记，保持原值（由 read-modify-write 补）
        legacy_migrated: existing.legacy_migrated,
    };
    data_dir::write_pointer(&ctx.default_data_dir, &pointer)
}
