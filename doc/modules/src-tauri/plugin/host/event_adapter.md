# event_adapter

## 职责
`CoreEvent` → `PluginEvent` 适配器 —— 插件事件边界的**唯一**转换点。职责（`PluginHost` 只负责驱动它，不重复实现其中任何一项）：

- **白名单**：不在协议内的事件直接丢弃（当前唯一被排除的是 `CoreEvent::TokenRotated`）；
- **字段裁剪 / 脱敏**：只搬运协议声明的字段（`AnnouncementChanged` 只带数量、`SessionLoggedIn` 不带用户 UUID），因此敏感内容**在类型层面**就无法外泄；
- **协议版本**：每条 envelope 都带 `PLUGIN_EVENT_PROTOCOL_VERSION`；
- **单调序号**：每个插件订阅持有一个适配器实例，序号连续，插件可据此自查缺口。

本模块**不**创建任何 channel、不持有总线、不做分发 —— 那些是 `PluginHost` 与 `core::event` 的职责。

## 核心类型/函数
- **`PluginEventAdapter`** — 单插件订阅的事件适配器（`Debug + Default`），唯一字段 `sequence: u64`。每个插件一个实例：序号是**每订阅**的连续计数，插件侧据此检测缺口
  - `new() -> Self`（`#[must_use]`）— 序号从 0 起（首个事件为 1）
  - `adapt(&mut self, event: &CoreEvent) -> Option<PluginEventEnvelope>` — 经 `to_plugin_event` 映射后包装为信封；**白名单外返回 `None` 且不消耗序号** —— 否则插件会看到「序号跳号但没收到任何事件」的假缺口
  - `resync_notice(&mut self, lost: u64) -> PluginEventEnvelope` — 生成协议级重新同步通知（`PluginEvent::ResyncRequired { lost }`），由 `PluginHost` 在 `Lagged` / 订阅建立时下发
  - `sequence(&self) -> u64`（`#[must_use]`）— 当前序号（已下发的最后一条）
  - `wrap(&mut self, event: PluginEvent) -> PluginEventEnvelope`（私有）— 递增序号并填充 `version` / `occurred_at`
- **`to_plugin_event(event: &CoreEvent) -> Option<PluginEvent>`**（私有自由函数）— **白名单映射的唯一来源**（返回 `None` 表示该核心事件不对插件开放）：
  - 直通：`OjSwitched`、`ContestSelected`、`ProblemOpened`、`SubmissionCreated`、`ConfigChanged`、`ThemeChanged`
  - 裁剪：`AnnouncementChanged { contest_id, new_ids }` → 只取 `new_count = new_ids.len()`；`LoggedIn { oj_id, .. }` → `SessionLoggedIn { oj_id }`（**丢弃用户 UUID**）；`LoggedOut` / `SessionExpired` → 同名会话事件；`SubmissionJudged` → `submission_id` + `status` 摘要；`WorkspaceSaved` → `workspace_id` / `revision` / `automatic`（**不含**文件内容与路径）
  - 排除：`CoreEvent::TokenRotated { .. } => return None` —— 凭证轮换是内部安全生命周期事实，对插件无价值且扩大攻击面
- **`utc_now_ms() -> i64`**（私有）— 当前 UTC 毫秒时间戳（`SystemTime::now().duration_since(UNIX_EPOCH)`，异常时 `unwrap_or_default()`）

## 直接依赖
- `core::event::core_event::CoreEvent`
- `plugin::api::event::{PluginEvent, PluginEventEnvelope, PLUGIN_EVENT_PROTOCOL_VERSION}`

## 被依赖
- `plugin::host::plugin_host`（每个订阅持有一个适配器实例，`adapt` / `resync_notice` 是投递路径的唯一入口）

## 逻辑流程
```
PluginHost::dispatch_to_subscribers(subs, event)
  → 每个 Subscription 的 adapter.adapt(event)
      ├─ to_plugin_event(event) ── None ──► 丢弃（不消耗序号）
      └─ Some(plugin_event) → wrap()：sequence += 1、version、occurred_at → envelope
  → sink.deliver(&envelope)

PluginHost 的 resync 回调
  → adapter.resync_notice(lost) → envelope(ResyncRequired { lost })
```

## 设计约束
- **白名单而非黑名单**：新增 `CoreEvent` 变体不会自动对插件可见，必须显式在此映射 —— 默认「不外泄」比默认「外泄后补救」安全得多。
- **脱敏发生在类型层**：插件拿不到源事件，只拿到协议声明过的字段，因此不可能「顺手」把用户 UUID、凭证、源代码带出去。
- **序号不因丢弃而跳号**：插件据 `sequence` 连续性与 `ResyncRequired` 互为补充地检测缺口（前者插件侧兜底，后者宿主侧主动通知）。
- **不持有总线、不建 channel**：适配器是纯函数式的转换器（除序号状态外无副作用），因此可独立单测，也不会形成引用环。

## 测试
`tests/event_adapter_tests.rs`：白名单事件逐一映射且字段被裁剪（`AnnouncementChanged` 只带数量、`LoggedIn` 不带 `user_id`、`WorkspaceSaved` 不带内容）、`TokenRotated` 被过滤且**不消耗序号**、序号从 1 起单调递增、`resync_notice` 带版本与序号、**全部适配结果不含敏感内容**（对 envelope 做字符串扫描，防止将来往协议里塞 token / 路径）、`occurred_at` 是毫秒时间戳。
