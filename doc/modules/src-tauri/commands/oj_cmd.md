# oj_cmd

> 源文件：`src-tauri/src/commands/oj_cmd.rs`

## 职责
OJ 切换 Command 模块，仅暴露 `switch_oj` 一个 IPC 命令。OJ 选择是应用级状态（落 `oj.active`），不是某次登录的参数 —— 旧版 `login(username, password, ojType?)` 内部 `set_current_oj` 的副作用能力悬空（前端从不传参），且切换后无人能观察；显式化后一个意图一个命令。

## 核心类型/函数
- `pub async fn switch_oj(ctx, oj_id: String) -> AppResult<()>` — 切换当前 OJ。前端 invoke 签名 `switch_oj`({ ojId })。**整段**——补注册、可用性检查、Registry 切换、缓存清理、`oj.active` 落盘、事件发布——在同一把 `ConfigService` 写事务锁内完成（评审三十三 #1，详见下）。编排五件事（顺序有意）：
  0. **按需补注册**（`ctx.ensure_oj_registered(id.as_str())`）：设置页允许从 OJ 枚举里挑一个尚未配置的类型、填地址保存后立即切换，而注册只在启动时发生 —— 不补注册用户就得重启客户端，UI 上表现为「切换失败：OJ 未注册」。注册条件与启动同源（只认配置里已启用且 id 匹配的实例，共用纯函数 `enabled_instance`），不能凭空激活未配置的 OJ
  1. 校验目标 OJ 已注册（`provider_registry.list_available().contains(&id)`）**且仍在当前配置的启用实例列表中**（注册表不会因配置删除/禁用而反注册，只查注册表会允许切到刚被保存移除的实例），未注册直接报 `AppError::ProviderNotFound`（不静默回退 —— 显式命令要显式结果）
  2. `set_current` 切换 Registry 当前 OJ，**紧接着显式清理各 Service 的 OJ 相关缓存**：`contest.on_oj_switched()` / `problem.on_oj_switched()` / `submission.on_oj_switched()`。**与 `set_current` 相邻、中间不夹可失败操作**：Registry 一旦切换，缓存必须同步失效，否则旧 OJ 数据会继续服务新 OJ 的查询
  3. 经 `ConfigService::update_locked` 持久化 `oj.active`（调用方已持有写事务锁）；失败如实上报「OJ 切换已生效但保存配置失败」（前端可提示重启后回退）—— 放在清缓存之后，保证缓存失效不依赖持久化结果
  4. 发布 `CoreEvent::OjSwitched { oj_id }` —— **纯事实通知**，供审计 / 插件 / 前端等观察者感知；Registry 与内存已切换，因此**即使 `oj.active` 落盘失败（`OjSwitchPersistFailed`）也必须发布**（否则插件宿主/审计不知道切换发生）。`ConfigChanged` 仍只由落盘成功的 update_locked 发布

## 直接依赖
- `tauri::State`
- `tracing::info`
- `crate::core::context::AppContext`
- `crate::core::error::{AppError, AppResult}`
- `crate::core::event::core_event::CoreEvent`
- `crate::core::provider::oj_id::OjId`

## 被依赖
- `src-tauri/src/commands/mod.rs`（`pub mod oj_cmd` 声明）
- `src-tauri/src/main.rs`（`tauri::generate_handler!` 注册）
- 前端 `src/bridge/config.bridge.ts`（`switchOj`，由 `services/config.service.ts` 编排）

## 逻辑流程
1. 前端 `invoke('switch_oj', { ojId })`（设置页「当前 OJ」下拉的显式动作）
2. `OjId::new` 规整入参（trim）→ **取得配置写事务锁** → 补注册 → 注册/配置校验 → `set_current` → `info!` 日志 → 三个 Service 的 `on_oj_switched()` → 持久化 `oj.active` → 发布 `CoreEvent::OjSwitched` → 释放事务锁
3. 错误以 `AppError` 返回（`ProviderNotFound` / `Config`），经 serde 序列化为 `{ Variant: msg }`，前端在 `bridge/index.ts` 归一化为 `IpcError`
4. 前端在**调用点**编排切换后果（`useSettingsForm.onSwitchOj` 成功分支）：`resetSessionForOjSwitch()` 清空旧 OJ 的会话与领域状态 → 导航登录页 → 路由守卫按新 OJ 的会话文件恢复会话

## 设计要点
- **清缓存是显式调用而不是事件订阅**：缓存清理属于「切换正确性的一部分」—— `switch_oj` 返回后新 OJ 的查询立刻可能进来，若清理要等异步事件消费者，就存在「旧 OJ 数据继续服务新 OJ 查询」的脏读窗口。旧实现靠同步投递把这件事做实，但那个保证建立在「发布方与订阅者同栈执行」的隐含前提上 —— 换成 `broadcast` 后该前提不再成立，因此改为显式调用，`OjSwitched` 只用来通知其他观察者。三个 Service 各自负责自己那批缓存（内存段同步清；磁盘段是空间回收，缓存键已带 OJ 维度，异步清理不影响正确性）。
- **前端状态由调用点直接重置**：切换是前端发起的命令，发起方编排后果（与登出同款模式），**不为 `OjSwitched` 引入 Tauri 事件桥**（`app.emit` + `@tauri-apps/api/event` 监听）。若未来出现非前端发起的切换（如配置热重载改 `oj.active`），再补事件桥把 `OjSwitched` 下发到 webview。
- **会话文件按 OJ 隔离是特性**：`sessions/{id}.json` 各自独立，切换不清旧 OJ 的登录态 —— 切回旧 OJ 免登录。也因此前端重置**不能走 `authStore.logout()`**（它会让已切换的 Registry 删新 OJ 的会话文件）。

统一AccessTransition覆盖Registry切换与比赛/其他Service清理；switch_scope组合发布与reset，且只覆盖这一段有效作用域变更。持久化`oj.active`与发布`OjSwitched`在access guard释放之后执行，不再把插件查询挡在写盘/广播期间；`OjSwitched` 在持久化结果之前发布，落盘失败也照发（ConfigChanged 不补发）。整段切换仍持有**配置写事务锁**（与 `update_config`/`reload` 同锁、锁序固定为「写事务锁 → config RwLock」），因此并发切换只能串行落地，不会出现「Registry=B、内存/磁盘=A」的永久分叉。回归用例 `concurrent_switches_stay_consistent_across_registry_memory_and_disk` 用事务内暂停点强制交错，并同时断言 Registry、内存与磁盘。

- `pub(crate) async fn switch_oj_impl(ctx: &AppContext, oj_id: &str)` — 切换编排本体（M2 提取）；内部 `switch_oj_transaction` 为同步事务本体（整段持有写事务锁），`#[cfg(test)] switch_oj_with_hook` / `switch_oj_with_hooks` 注入「Registry 切换后」「取锁前」「持久化完成后」探针（测试见 `tests/oj_cmd_tests.rs`：注册/配置校验、registry 切换、配置落盘、`OjSwitched` 事件、并发一致性）。并发用例用 `write_txn_is_held` 的 `try_lock` 探测做确定性握手；T2 的完成信号在**持久化完成之后**（`after_persist`）才发出，移除事务锁时 T1 会等 T2 真正写盘后再写 Hydro，旧交错必然出现，不依赖 sleep
