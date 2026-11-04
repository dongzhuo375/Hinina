use serde::Serialize;
use tauri::State;
use tracing::{info, warn};

use crate::commands::oj_cmd::switch_scope;
use crate::core::context::AppContext;
use crate::core::entity::config::AppConfig;
use crate::core::error::{AppError, AppResult};
use crate::core::event::core_event::CoreEvent;
use crate::core::provider::oj_id::OjId;
use crate::infra::launch_args;
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
    let config = reload_config_impl(&ctx)?;
    Ok(config)
}

/// `reload_config` 的真实编排（提取供测试直接调用）。
///
/// **先在事务内预检、通过后才提交内存与事件**：旧顺序先
/// `reload_locked` 替换内存并发布 `ConfigChanged`，再校验 active 能否注册/可用；
/// 磁盘指向一个没有适配器的已启用实例（如 Ghost）时，校验失败返回错误，但
/// 内存/磁盘已是 Ghost、Registry 仍是旧 OJ，且事件已发出。
///
/// 现在 `load_normalized` 只读盘+归一（不提交），先做 **active 指向已启用实例**
/// 的结构检查（与 `registry_changed` 无关——归一化可能回退默认值），再用磁盘配置补注册
/// 并做可用性预检；失败时内存、Registry、事件全部保持原样。通过后用**外层访问
/// 栅栏**覆盖「提交内存/ConfigChanged + 切 Registry/清 OJ 缓存/OjSwitched」整段，
/// 插件读者不会看到「新配置 + 旧 Registry」的半提交态。
pub(crate) fn reload_config_impl(ctx: &AppContext) -> AppResult<AppConfig> {
    reload_config_impl_inner(ctx, || {})
}

/// 测试专用：在配置已提交、Registry 尚未切换的窗口内注入观察点。
#[cfg(test)]
pub(crate) fn reload_config_impl_with_hook<F>(
    ctx: &AppContext,
    after_commit: F,
) -> AppResult<AppConfig>
where
    F: FnOnce(),
{
    reload_config_impl_inner(ctx, after_commit)
}

fn reload_config_impl_inner<F>(ctx: &AppContext, after_commit: F) -> AppResult<AppConfig>
where
    F: FnOnce(),
{
    let _txn = ctx.config.lock_write_txn();
    let config = ctx.config.load_normalized()?;
    let active = config.oj.active.clone();
    // 归一化在「零启用实例」时会回退默认 HOJ（即使 HOJ 被禁用/不存在），
    // 因此不能把「active 已启用」当作归一化的保证；必须无条件检查。
    if !config
        .oj
        .instances
        .iter()
        .any(|instance| instance.enabled && instance.id == active)
    {
        return Err(AppError::Config(format!(
            "当前 OJ（{}）不在已启用的实例列表中",
            active
        )));
    }
    let registry_changed = ctx.provider_registry.current_id().as_str() != active;
    if registry_changed {
        // 用磁盘配置预检（此时尚未提交内存）：目标可能没有适配器工厂。
        ctx.ensure_oj_registered_from(&config, &active);
        if !ctx
            .provider_registry
            .list_available()
            .contains(&OjId::new(&active))
        {
            return Err(AppError::ProviderNotFound(format!(
                "配置的当前 OJ {} 未注册（需在 config.json 的 oj.instances 中启用对应实例）",
                active
            )));
        }
    }

    // 外层访问栅栏：配置提交与 Registry 同步是一个访问变更，读者不得观察到
    // 「新配置 + 旧 Registry」的混合态。
    let _transition = ctx.access.begin();
    // 预检通过：提交内存并发布 ConfigChanged
    ctx.config.commit_reloaded(&config);
    after_commit();
    if registry_changed {
        switch_scope(
            &ctx.access,
            ctx.provider_registry.as_ref(),
            OjId::new(&active),
            || {
                ctx.contest.on_oj_switched();
                ctx.problem.on_oj_switched();
                ctx.submission.on_oj_switched();
            },
        );
        info!(oj_id = %active, "配置重载：Registry 已同步到磁盘 active");
        ctx.event_bus
            .publish(CoreEvent::OjSwitched { oj_id: active });
    }
    // 显式同步（幂等）
    crate::commands::workspace_cmd::sync_auto_save_from_context(ctx);
    Ok(config)
}

/// 更新配置并持久化。
///
/// 前端 invoke 签名: `update_config`({ config })
///
/// **`oj.active` 无论哪种模式都只由 `switch_oj` 改写**：载荷里的 active 可能是
/// 保存开始时的旧快照，与在途切换交错时会把新 active 覆盖回去，
/// 这里一律以内存当前值为准。
///
/// **锁定执行真值由后端自持**：`--settings` 由组合根在启动早期固化进
/// `launch_args`（普通启动 false → 锁定字段走保留/放宽语义；钥匙启动 true →
/// 维护者可写全部字段）。前端不传「是否锁定」——UI 层的锁定判断只影响可编辑性，
/// 没有任何 IPC 输入能改变后端的执行路径。
///
/// 持久化前做后端兜底校验（config.json 可被手改，前端不是唯一防线），两条路径：
/// - **钥匙视图**（`maintainer_locked=false`）：先 `validate()` 拒绝
///   不可钳制项（服务器地址非法），再 `sanitize()` 把越界字段钳制到与前端
///   SettingsView 一致的取值域。
/// - **普通启动**（`maintainer_locked=true`）：先用旧配置覆盖锁定字段
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
/// 主题域（`set_theme`/`get_theme`）已随死代码清理删除：前端只有编辑器主题，
/// 经 `update_config` 落盘并由消费方本地应用；此处不做额外广播。
#[tauri::command]
pub async fn update_config(ctx: State<'_, AppContext>, config: AppConfig) -> AppResult<AppConfig> {
    info!("Command: 更新配置");
    // 分层锁的执行点必须在**持有真值的一侧**：`--settings` 只由组合根固化，
    // 前端不参与该判断（一次进程生命周期内不可变）。
    let maintainer_locked = !launch_args::get().settings;
    apply_config_update(&ctx, config, maintainer_locked)
}

/// `update_config` 的真实编排（提取供测试直接调用，`#[tauri::command]` 只做薄转发）。
///
/// `maintainer_locked` 是**执行真值**（组合根从 launch_args 推导；测试显式传布尔
/// 模拟两种启动模式，不经 launch_args——它是不可重置的进程级 OnceLock）：为真时
/// 走「锁定字段不动」语义：先
/// `preserve_locked_fields` 用旧配置覆盖锁定字段，再用放宽后的校验/净化 ——
/// 手改出的非法锁定值（地址、越界数值）不得阻塞选手层字段的保存，也不得被
/// 静默钳制/改写。
pub(crate) fn apply_config_update(
    ctx: &AppContext,
    config: AppConfig,
    maintainer_locked: bool,
) -> AppResult<AppConfig> {
    apply_config_update_inner(
        ctx,
        config,
        maintainer_locked,
        &mut || {},
        &mut crate::commands::workspace_cmd::sync_auto_save_from_context,
    )
}

/// `apply_config_update` 的真实实现；`before_validate`/`sync` 只供测试注入。
///
/// **整个「读最新活 → 合并/锁定 → 固定 active → 最终校验 → 整体替换 → 同步
/// auto-save」都在 `ConfigService` 的写事务锁内完成**（
/// 三十六 P2）：同步若在锁外，U1 保存 `autoSave=false` 与 U2 保存 `true`
/// 可以交错成「磁盘开启、管理器停止」（或反向）；把同步纳入同一串行域后，
/// 最后一次写入与它触发的启停严格同序。
pub(crate) fn apply_config_update_inner(
    ctx: &AppContext,
    mut config: AppConfig,
    maintainer_locked: bool,
    before_validate: &mut dyn FnMut(),
    sync: &mut dyn FnMut(&AppContext),
) -> AppResult<AppConfig> {
    let _txn = ctx.config.lock_write_txn();
    // `oj.active` **只由 switch_oj 改写**（保存载荷的 active 是开始时的快照）：调用方带来的 active
    // 可能是保存开始时的旧快照，与在途切换交错时会把新 active 覆盖回去。
    // 这里在写锁闭包内取**锁内最新值**，两种模式一律以它为准。
    let saved = ctx.config.update_locked(|cfg| {
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

    // 同步仍在事务内：与本次提交严格同序
    sync(ctx);
    Ok(saved)
}

/// 测试专用：保留 `before_validate` 注入点（并发保存/切换回归）。
#[cfg(test)]
pub(crate) fn apply_config_update_with_hook(
    ctx: &AppContext,
    config: AppConfig,
    maintainer_locked: bool,
    before_validate: &mut dyn FnMut(),
) -> AppResult<AppConfig> {
    apply_config_update_inner(
        ctx,
        config,
        maintainer_locked,
        before_validate,
        &mut crate::commands::workspace_cmd::sync_auto_save_from_context,
    )
}

/// 测试专用：在 auto-save 同步**读取配置之后、执行启停之前**注入暂停点，
/// 用于确定性地复现「旧快照的同步反向覆盖新配置」。
#[cfg(test)]
pub(crate) fn apply_config_update_with_sync_hook(
    ctx: &AppContext,
    config: AppConfig,
    maintainer_locked: bool,
    after_sync_read: &mut dyn FnMut(),
) -> AppResult<AppConfig> {
    apply_config_update_inner(ctx, config, maintainer_locked, &mut || {}, &mut |ctx| {
        crate::commands::workspace_cmd::sync_auto_save_from_context_with_hook(ctx, || {
            after_sync_read()
        });
    })
}
