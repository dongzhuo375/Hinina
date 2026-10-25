# permission/mod

## 职责
插件授权快照：宿主批准权限与清单声明的不可变交集，以及事件 allowlist 的唯一来源。

## 核心类型/函数
- **`PluginGrant`**（不可变快照）：
  - `authorize(manifest, approved)` — 构造：批准 ∩ 声明，非法交集拒绝
  - `require(permission) -> PluginResult<()>` — 每个 API 调用前的授权检查
  - `manifest() -> &PluginManifest` — 清单只读访问
  - `pub(crate) fn allowed_events(&self) -> impl Iterator<Item = PluginEventSubscription> + '_` — 事件 allowlist：过滤清单声明中缺少载荷权限的业务事件、`ResyncRequired` 只追加一次，无 events.subscribe 时迭代器为空；调用方（manager/plugin_host）自行 collect

## 直接依赖
- `plugin::api::{PluginError, PluginResult}`
- `plugin::host::manifest::{PluginEventSubscription, PluginManifest, PluginPermission}`
- `std::collections::HashSet`

## 被依赖
- `plugin::manager`（注册）、`plugin::host::plugin_host`（交付过滤）、`plugin::query` / `plugin::execution_api`（每次授权检查）

## 逻辑流程
- **交集而非声明**：清单不能自动授权；每个 API 和领域事件独立验证。运行中的授权不跟随磁盘配置静默扩张——编辑 plugins.json 后需 stop/reload 生效
- **权限表**：事件订阅需 events.subscribe；submission.* 需 submission.read；contest.* 需 contest.read（榜单另有 contest.rank）；identity.current 需 identity.read；storage 读写分别验证、CAS 需两者；network.request 还需精确 origin/地址批准；notification 独立授权；Resync 是强制控制通知（详见 [API](../api/mod.md)）
- **allowed_events**：Manager 注册与 Host 交付共用的唯一过滤——无 events.subscribe 返回空；有授权时过滤缺少领域读取权限的业务事件，并保证 resync.required 只加入一次
