use serde::Serialize;
use tauri::State;
use tracing::{info, warn};

use crate::core::context::AppContext;
use crate::core::entity::config::AppConfig;
use crate::core::error::{AppError, AppResult};
use crate::infra::logger::LOG_RELATIVE_PATH;

/// 存储与版本信息（排障用：前端「关于/诊断」面板展示日志位置与客户端版本）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    /// 应用数据根目录
    pub base_dir: String,
    /// 日志文件完整路径（由 `infra::logger::LOG_RELATIVE_PATH` 拼出，与落盘位置同源）
    pub log_path: String,
    /// 客户端版本（构建时注入的 Cargo 包版本，与 HttpClient UA 同源）
    pub version: String,
}

/// 获取存储与版本信息。
///
/// 前端 invoke 签名: `get_storage_info`
#[tauri::command]
pub async fn get_storage_info(ctx: State<'_, AppContext>) -> AppResult<StorageInfo> {
    let base_dir = ctx.storage.base_dir();
    Ok(StorageInfo {
        log_path: base_dir
            .join(LOG_RELATIVE_PATH)
            .to_string_lossy()
            .into_owned(),
        base_dir: base_dir.to_string_lossy().into_owned(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// 获取完整应用配置。
///
/// 前端 invoke 签名: `get_config`
///
/// 返回 `AppConfig`（包含 user/oj/editor/theme/layout 五个子分组）。
/// 配置源自磁盘持久化文件，首次启动时自动生成默认值。
#[tauri::command]
pub async fn get_config(ctx: State<'_, AppContext>) -> AppResult<AppConfig> {
    Ok(ctx.config.get())
}

/// 从磁盘重新加载配置。
///
/// 前端 invoke 签名: `reload_config`
///
/// 落盘/内存更新完成后**显式**按新配置同步 auto-save 的启停与间隔，
/// 并发布 `CoreEvent::ConfigChanged` 作为纯通知。
#[tauri::command]
pub async fn reload_config(ctx: State<'_, AppContext>) -> AppResult<AppConfig> {
    info!("Command: 重新加载配置");
    let config = ctx.config.reload()?;
    crate::commands::workspace_cmd::sync_auto_save_from_context(&ctx);
    Ok(config)
}

/// 更新配置并持久化。
///
/// 前端 invoke 签名: `update_config`({ config, maintainerLocked })
///
/// **`oj.active` 无论哪种模式都只由 `switch_oj` 改写**：载荷里的 active 可能是
/// 保存开始时的旧快照，与在途切换交错时会把新 active 覆盖回去（评审二十九 #1），
/// 这里一律以内存当前值为准。
///
/// 持久化前做后端兜底校验（config.json 可被手改，前端不是唯一防线），两条路径：
/// - **普通模式**（`maintainer_locked=false`，钥匙视图）：先 `validate()` 拒绝
///   不可钳制项（服务器地址非法），再 `sanitize()` 把越界字段钳制到与前端
///   SettingsView 一致的取值域。
/// - **锁定模式**（`maintainer_locked=true`，普通启动）：先用旧配置覆盖锁定字段
///   （`preserve_locked_fields`），再 `validate_preserving_locked`（只查 active
///   指向已启用实例）与 `sanitize_editable`（只钳制可编辑层）——手改出的非法
///   锁定值既不阻塞可编辑字段保存，也不被改写。
///
/// **配置变更后的必要动作在此显式完成**：`ConfigService::update` 落盘成功后，
/// 本命令立即按新配置同步 auto-save 的启停与间隔 —— 不依赖事件消费者。
/// 旧实现把这件事挂在 `ConfigReloaded` 事件的订阅者上（订阅者内部还要
/// `tauri::async_runtime::spawn` 才拿得到 tokio 上下文），消费者一旦落后或缺失，
/// 设置页改了开关却不生效，且无从察觉。
///
/// 主题字段的切换效果由前端本地应用（`set_theme` 另有命令）；此处不做额外广播。
#[tauri::command]
pub async fn update_config(
    ctx: State<'_, AppContext>,
    config: AppConfig,
    // 维护者层锁定（普通启动）：锁定字段取旧值、不参与校验与钳制
    maintainer_locked: Option<bool>,
) -> AppResult<()> {
    info!("Command: 更新配置");
    apply_config_update(&ctx, config, maintainer_locked.unwrap_or(false))
}

/// `update_config` 的真实编排（提取供测试直接调用，`#[tauri::command]` 只做薄转发）。
///
/// `maintainer_locked` 为真时走「锁定字段不动」语义（评审二十六 #2）：先
/// `preserve_locked_fields` 用旧配置覆盖锁定字段，再用放宽后的校验/净化 ——
/// 手改出的非法锁定值（地址、越界数值）不得阻塞选手层字段的保存，也不得被
/// 静默钳制/改写。
pub(crate) fn apply_config_update(
    ctx: &AppContext,
    config: AppConfig,
    maintainer_locked: bool,
) -> AppResult<()> {
    apply_config_update_with_hook(ctx, config, maintainer_locked, &mut || {})
}

/// `apply_config_update` 的真实实现；`before_validate` 只供测试注入强制交错。
///
/// **整个「读最新活 → 合并/锁定 → 固定 active → 最终校验 → 整体替换」都在
/// `ConfigService` 的写事务锁内完成**（评审三十三 #2）：旧实现先用载荷里的
/// 旧 active 校验、再在写锁内换成最新 active 后直接落盘——若交错切换把 active
/// 指向载荷中已删除/禁用的实例，就会保存一份 `AppConfig::validate()` 拒绝的配置。
/// 校验必须看到**最终候选**，并与 `switch_oj` 共用同一把锁。
pub(crate) fn apply_config_update_with_hook(
    ctx: &AppContext,
    mut config: AppConfig,
    maintainer_locked: bool,
    before_validate: &mut dyn FnMut(),
) -> AppResult<()> {
    ctx.config.update_checked(|cfg| {
        // `oj.active` **只由 switch_oj 改写**（评审二十九 #1）：调用方带来的 active
        // 可能是保存开始时的旧快照，与在途切换交错时会把新 active 覆盖回去。
        // 这里在写锁闭包内取**锁内最新值**，两种模式一律以它为准。
        let previous = cfg.clone();
        config.oj.active = previous.oj.active.clone();
        // 测试注入点：此刻写事务已持有，并发的 switch_oj 必须被挡在事务外
        before_validate();
        if maintainer_locked {
            config.preserve_locked_fields(&previous);
            config
                .validate_preserving_locked()
                .map_err(AppError::Config)?;
            if config.sanitize_editable() {
                warn!("配置存在可编辑层越界字段，已钳制后持久化");
            }
        } else {
            config.validate().map_err(AppError::Config)?;
            if config.sanitize() {
                warn!("配置存在越界字段，已按取值域钳制后持久化: {:?}", config);
            }
        }
        // 校验/净化针对的是**固定 active 之后的最终候选**；失败时事务回滚，
        // 内存与磁盘都保持原样
        *cfg = config;
        Ok(())
    })?;

    // 显式同步（幂等）：间隔未变时不重启计时器，关掉后也能停
    crate::commands::workspace_cmd::sync_auto_save_from_context(ctx);
    Ok(())
}
