// OJ 切换 Command。
//
// OJ 选择是应用级状态（落 `oj.active`），不是某次登录的参数 —— 旧版
// `login(username, password, ojType?)` 内部 `set_current_oj` 的副作用能力
// 悬空（前端从不传参），且切换后无人能观察。显式化后：一个意图一个命令。

use tauri::State;
use tracing::info;

use crate::core::context::AppContext;
use crate::core::error::{AppError, AppResult};
use crate::core::event::core_event::CoreEvent;
use crate::core::provider::oj_id::OjId;

/// 切换当前 OJ。
///
/// 前端 invoke 签名: `switch_oj`({ ojId })
///
/// 编排四件事（顺序有意）：
/// 0. **按需补注册**（`ensure_oj_registered`）：设置页允许从枚举里挑一个尚未配置的
///    OJ、填地址保存后立即切换，而注册只在启动时发生 —— 不补注册就得重启客户端；
/// 1. 校验目标 OJ 已注册（未注册直接报错，不静默回退 —— 显式命令要显式结果）；
/// 2. 切换 Registry 当前 OJ，**紧接着显式清理各 Service 的 OJ 相关缓存**；
/// 3. 持久化 `oj.active` —— 失败如实上报（切换已生效，重启后回退），但不影响第 2 步；
/// 4. 最后发布 `CoreEvent::OjSwitched` —— **纯事实通知**。
///
/// # 为什么清缓存是显式调用而不是事件订阅
///
/// 缓存清理属于「切换正确性的一部分」：`switch_oj` 返回后新 OJ 的查询立刻可能进来，
/// 若清理要等异步事件消费者，就存在「旧 OJ 数据继续服务新 OJ 查询」的脏读窗口。
/// 旧实现靠 `EventBus` 的同步投递把这件事做实，但那个保证建立在「发布方与订阅者
/// 同栈执行」的隐含前提上 —— 换成 `broadcast` 后该前提不再成立，因此改为显式调用，
/// `OjSwitched` 只用来通知其他观察者（审计、插件、前端）。
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

    // 与 set_current 相邻、中间不夹可失败操作：Registry 一旦切换，缓存必须同步失效。
    // 三个 Service 各自负责自己那批缓存（内存段同步清；磁盘段是空间回收，
    // 缓存键已带 OJ 维度，异步清理不影响正确性）。
    ctx.contest.on_oj_switched();
    ctx.problem.on_oj_switched();
    ctx.submission.on_oj_switched();

    ctx.config
        .update(|draft| {
            draft.oj.active = id.to_string();
        })
        .map_err(|e| {
            // 切换已生效但持久化失败：如实上报，前端可提示重启后回退
            AppError::Config(format!("OJ 切换已生效但保存配置失败: {}", e))
        })?;

    ctx.event_bus.publish(CoreEvent::OjSwitched {
        oj_id: id.to_string(),
    });

    Ok(())
}
