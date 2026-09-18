// OJ 切换 Command。
//
// OJ 选择是应用级状态（落 `oj.active`），不是某次登录的参数 —— 旧版
// `login(username, password, ojType?)` 内部 `set_current_oj` 的副作用能力
// 悬空（前端从不传参），且切换后无人能观察。显式化后：一个意图一个命令。

use tauri::State;
use tracing::info;

use crate::core::context::AppContext;
use crate::core::error::{AppError, AppResult};
use crate::core::event::app_event::{AppEvent, SystemEvent};
use crate::core::provider::oj_id::OjId;

/// 切换当前 OJ。
///
/// 前端 invoke 签名: `switch_oj`({ ojId })
///
/// 编排三件事（顺序有意）：
/// 1. 校验目标 OJ 已注册（未注册直接报错，不静默回退 —— 显式命令要显式结果）；
/// 2. 切换 Registry 当前 OJ 并持久化 `oj.active`；
/// 3. 发布 `OJSwitched`（状态变更走事件，符合 EventBus 原则）。
#[tauri::command]
pub async fn switch_oj(ctx: State<'_, AppContext>, oj_id: String) -> AppResult<()> {
    let id = OjId::new(&oj_id);
    if !ctx.provider_registry.list_available().contains(&id) {
        return Err(AppError::ProviderNotFound(format!(
            "OJ {} 未注册（需在 config.json 的 oj.instances 中启用对应实例）",
            id
        )));
    }

    ctx.provider_registry.set_current(id.clone());
    ctx.config
        .update(|draft| {
            draft.oj.active = id.to_string();
        })
        .map_err(|e| {
            // 切换已生效但持久化失败：如实上报，前端可提示重启后回退
            AppError::Config(format!("OJ 切换已生效但保存配置失败: {}", e))
        })?;

    info!(oj_id = %id, "已切换当前 OJ");
    ctx.event_bus
        .publish(&AppEvent::System(SystemEvent::OJSwitched { oj_id: id.to_string() }));
    Ok(())
}
