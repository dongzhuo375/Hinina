# oj_cmd

> 源文件：`src-tauri/src/commands/oj_cmd.rs`

## 职责
OJ 切换 Command 模块，仅暴露 `switch_oj` 一个 IPC 命令。OJ 选择是应用级状态（落 `oj.active`），不是某次登录的参数 —— 旧版 `login(username, password, ojType?)` 内部 `set_current_oj` 的副作用能力悬空（前端从不传参），且切换后无人能观察；显式化后一个意图一个命令。

## 核心类型/函数
- `pub async fn switch_oj(ctx, oj_id: String) -> AppResult<()>` — 切换当前 OJ。前端 invoke 签名 `switch_oj`({ ojId })。编排三件事（顺序有意）：
  1. 校验目标 OJ 已注册（`provider_registry.list_available().contains(&id)`），未注册直接报 `AppError::ProviderNotFound`（不静默回退 —— 显式命令要显式结果）
  2. `set_current` 切换 Registry 当前 OJ，并经 `ConfigService::update` 持久化 `oj.active`；持久化失败如实上报「OJ 切换已生效但保存配置失败」（前端可提示重启后回退）
  3. 发布 `SystemEvent::OJSwitched { oj_id }`（状态变更走事件，符合 EventBus 原则）

## 直接依赖
- `tauri::State`
- `tracing::info`
- `crate::core::context::AppContext`
- `crate::core::error::{AppError, AppResult}`
- `crate::core::event::app_event::{AppEvent, SystemEvent}`
- `crate::core::provider::oj_id::OjId`

## 被依赖
- `src-tauri/src/commands/mod.rs`（`pub mod oj_cmd` 声明）
- `src-tauri/src/main.rs`（`tauri::generate_handler!` 注册）
- 前端 `src/bridge/config.bridge.ts`（`switchOj`，由 `services/config.service.ts` 编排）

## 逻辑流程
1. 前端 `invoke('switch_oj', { ojId })`（设置页「当前 OJ」下拉的显式动作）
2. `OjId::new` 规整入参（trim）→ 注册校验 → 切换 Registry + 持久化 `oj.active` → `info!` 日志 → 发布 `OJSwitched`
3. 错误以 `AppError` 返回（`ProviderNotFound` / `Config`），经 serde 序列化为 `{ Variant: msg }`，前端在 `bridge/index.ts` 归一化为 `IpcError`
