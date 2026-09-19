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
/// 0. **按需补注册**（`ensure_oj_registered`）：设置页允许从枚举里挑一个尚未配置的
///    OJ、填地址保存后立即切换，而注册只在启动时发生 —— 不补注册就得重启客户端；
/// 1. 校验目标 OJ 已注册（未注册直接报错，不静默回退 —— 显式命令要显式结果）；
/// 2. 切换 Registry 当前 OJ，**紧接着**发布 `OJSwitched`（缓存失效不得依赖后续
///    可能失败的操作：`set_current` 已生效即必须清缓存，否则旧 OJ 数据会继续服务
///    新 OJ 的查询）；
/// 3. 持久化 `oj.active` —— 失败如实上报（切换已生效，重启后回退），但不影响第 2 步。
#[tauri::command]
pub async fn switch_oj(ctx: State<'_, AppContext>, oj_id: String) -> AppResult<()> {
    let id = OjId::new(&oj_id);
    // 新配置的实例可能尚未注册（注册只在启动时发生）——先补注册，再校验
    ctx.ensure_oj_registered(id.as_str());
    if !ctx.provider_registry.list_available().contains(&id) {
        return Err(AppError::ProviderNotFound(format!(
            "OJ {} 未注册（需在 config.json 的 oj.instances 中启用对应实例）",
            id
        )));
    }

    ctx.provider_registry.set_current(id.clone());
    info!(oj_id = %id, "已切换当前 OJ");
    // 与 set_current 相邻、中间不夹可失败操作：Registry 一旦切换，缓存必须同步失效
    // （缓存键虽已带 OJ 维度、正确性不依赖它，但不清会让当前会话继续用旧 OJ 的数据）
    ctx.event_bus
        .publish(&AppEvent::System(SystemEvent::OJSwitched {
            oj_id: id.to_string(),
        }));

    ctx.config
        .update(|draft| {
            draft.oj.active = id.to_string();
        })
        .map_err(|e| {
            // 切换已生效但持久化失败：如实上报，前端可提示重启后回退
            AppError::Config(format!("OJ 切换已生效但保存配置失败: {}", e))
        })?;

    Ok(())
}
