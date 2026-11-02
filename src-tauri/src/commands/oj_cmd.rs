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

/// One access fence covers registry publication and all dependent scope resets.
pub(crate) fn switch_scope(
    access: &std::sync::Arc<crate::core::access::AccessRevision>,
    registry: &dyn crate::core::provider::registry::ProviderRegistry,
    id: OjId,
    reset: impl FnOnce(),
) {
    let _transition = access.begin();
    registry.set_current(id);
    reset();
}

/// 切换当前 OJ。
///
/// 前端 invoke 签名: `switch_oj`({ ojId })
///
/// 编排四件事（顺序有意，**整段在配置写事务锁内**）：
/// 0. **按需补注册**（`ensure_oj_registered`）：设置页允许从枚举里挑一个尚未配置的
///    OJ、填地址保存后立即切换，而注册只在启动时发生 —— 不补注册就得重启客户端；
/// 1. 校验目标 OJ 已注册**且仍在当前配置的启用实例列表中**（注册表不会因配置
///    删除/禁用而反注册；未注册直接报错，不静默回退 —— 显式命令要显式结果）；
/// 2. 切换 Registry 当前 OJ，**紧接着显式清理各 Service 的 OJ 相关缓存**；
/// 3. 持久化 `oj.active` —— 失败如实上报（切换已生效，重启后回退），但不影响第 2 步；
/// 4. 最后发布 `CoreEvent::OjSwitched` —— **纯事实通知**。
///
/// **为什么整段共用写事务锁**（评审三十三 #1）：只锁落盘会留下「switch_scope
/// 已切 Registry、尚未落盘」的窗口，两个并发切换可在其中交错出
/// 「Registry=B、内存/磁盘=A」的永久分叉；与 `update_config`/`reload` 共用
/// 同一把锁后，切换与保存也串行，保存的最终校验不会看到半途的 active。
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
    switch_oj_impl(&ctx, &oj_id).await
}

/// `switch_oj` 的真实编排（提取自命令本体，供命令与测试共用同一实现）。
pub(crate) async fn switch_oj_impl(ctx: &AppContext, oj_id: &str) -> AppResult<()> {
    switch_oj_transaction(ctx, &OjId::new(oj_id), |_| {}, || {}, || {})
}

/// 切换事务本体（同步）：**整段**——补注册、可用性检查、Registry 切换、缓存
/// 清理、`oj.active` 落盘、事件发布——都在 `ConfigService` 的写事务锁内完成。
///
/// 为什么必须整段加锁（评审三十三 #1）：只把落盘包进 `ConfigService::update`
/// 的锁里仍留出「`switch_scope` 已切 Registry、尚未落盘」的窗口。两个并发切换
/// 在这个窗口交错时（T1 切 A → T2 切 B 并落盘 B → T1 落盘 A），会得到
/// Registry=B、内存/磁盘=A 的**永久分叉**，而两次调用都返回成功。前端单飞
/// 不能替代后端约束（旧控制器/多窗口/直接 IPC 都可能绕过它）。
///
/// 锁序固定为「写事务锁 → config 的 RwLock」（`ensure_oj_registered` 与本次
/// 配置检查只读 `config`），与 `update_config`/`reload` 同序。
fn switch_oj_transaction<BeforeLock, AfterScope, AfterPersist>(
    ctx: &AppContext,
    id: &OjId,
    before_lock: BeforeLock,
    after_scope: AfterScope,
    after_persist: AfterPersist,
) -> AppResult<()>
where
    BeforeLock: FnOnce(&AppContext),
    AfterScope: FnOnce(),
    AfterPersist: FnOnce(),
{
    // 测试注入点（生产为 no-op）：在**尝试取锁之前**执行。并发回归用它探测
    // 「另一个线程是否已持有写事务」，从而不靠 sleep 猜测调度（评审三十四）。
    before_lock(ctx);
    let _txn = ctx.config.lock_write_txn();

    // 新配置的实例可能尚未注册（注册只在启动时发生）——先补注册，再校验
    ctx.ensure_oj_registered(id.as_str());
    if !ctx.provider_registry.list_available().contains(id) {
        return Err(AppError::ProviderNotFound(format!(
            "OJ {} 未注册（需在 config.json 的 oj.instances 中启用对应实例）",
            id
        )));
    }
    // **配置是「哪些 OJ 可用」的权威来源**：注册表不会因配置里删除/禁用实例而
    // 反注册。只查注册表会允许切到刚被保存移除的实例，落盘出的 `oj.active`
    // 指向不存在/禁用实例——正是 `AppConfig::validate()` 拒绝的形态
    //（评审三十三 #2 的交错序列需要这道检查才能闭合）。
    if !ctx
        .config
        .get()
        .oj
        .instances
        .iter()
        .any(|instance| instance.enabled && instance.id == id.as_str())
    {
        return Err(AppError::ProviderNotFound(format!(
            "OJ {} 未在已启用的实例列表中（需在 config.json 的 oj.instances 中启用对应实例）",
            id
        )));
    }

    // 栅栏只覆盖「发布新 OJ + 失效旧作用域缓存」这一段：持久化与事件通知不属于
    // 有效作用域变更，不应把插件查询挡在写盘/广播期间。switch_scope 内部自带
    // 同一个 guard，因此这里不能再套一层外层 guard。
    // 与 set_current 相邻、中间不夹可失败操作：Registry 一旦切换，缓存必须同步失效。
    // 三个 Service 各自负责自己那批缓存（内存段同步清；磁盘段是空间回收，
    // 缓存键已带 OJ 维度，异步清理不影响正确性）。
    switch_scope(
        &ctx.access,
        ctx.provider_registry.as_ref(),
        id.clone(),
        || {
            ctx.contest.on_oj_switched();
            ctx.problem.on_oj_switched();
            ctx.submission.on_oj_switched();
        },
    );
    info!(oj_id = %id, "已切换当前 OJ");

    // 测试注入点：此刻 Registry 已切换、写事务仍持有；测试用它强制第二个
    // 切换在「落盘之前」发起，验证它被挡在事务外（评审三十三 #1 的复现序列）
    after_scope();

    let save_result = ctx
        .config
        .update_locked(|draft| {
            draft.oj.active = id.to_string();
            Ok(())
        })
        .map_err(|e| {
            // 切换已生效但持久化失败：用**专用变体**上报，前端据此承认已生效
            // 而不是回滚 UI（评审二十四 #1）
            AppError::OjSwitchPersistFailed(e.to_string())
        });

    // Registry 与内存都已切换——`OjSwitched` 是**已发生的事实**，无论落盘
    // 成败都要发布，否则插件宿主/审计观察者不知道切换发生（评审三十六 P2）。
    // `ConfigChanged` 仍只由 update_locked 在落盘成功后发布。
    ctx.event_bus.publish(CoreEvent::OjSwitched {
        oj_id: id.to_string(),
    });

    // 持久化失败：事件已发，返回专用错误
    save_result?;
    // 测试注入点：**持久化已完成后**执行（评审三十五）。并发回归用它在旧实现
    // 下等第二个切换真正落盘，而不是在落盘前发信号造成假绿。
    after_persist();

    Ok(())
}

/// 仅测试使用：在 `switch_oj` 事务内、Registry 切换后注入暂停点。
#[cfg(test)]
pub(crate) fn switch_oj_with_hook<F>(ctx: &AppContext, oj_id: &str, after_scope: F) -> AppResult<()>
where
    F: FnOnce(),
{
    switch_oj_transaction(ctx, &OjId::new(oj_id), |_| {}, after_scope, || {})
}

/// 仅测试使用：注入「取锁前」「Registry 切换后」「持久化完成后」三个探针。
#[cfg(test)]
pub(crate) fn switch_oj_with_hooks<B, S, P>(
    ctx: &AppContext,
    oj_id: &str,
    before_lock: B,
    after_scope: S,
    after_persist: P,
) -> AppResult<()>
where
    B: FnOnce(&AppContext),
    S: FnOnce(),
    P: FnOnce(),
{
    switch_oj_transaction(
        ctx,
        &OjId::new(oj_id),
        before_lock,
        after_scope,
        after_persist,
    )
}

#[cfg(test)]
#[path = "tests/oj_cmd_tests.rs"]
mod tests;
