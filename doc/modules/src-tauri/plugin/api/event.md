# event

## 职责
插件事件协议 —— 插件侧**唯一**允许依赖的事件接口。定义插件可见的事件载荷 `PluginEvent`、随事件下发的信封 `PluginEventEnvelope` 与协议版本常量。转换与过滤发生在 `plugin::host::event_adapter`，由 `PluginHost` 统一驱动 —— **不存在第二套业务总线**。

**边界（硬约束）**：插件**不得**直接依赖 `CoreEvent`（内部事件载荷）、内部 Service 类型、领域实体、内部缓存结构，或 `tokio::sync::broadcast::Receiver<CoreEvent>`；只能收到经适配器白名单过滤、脱敏与版本化之后的 `PluginEventEnvelope`。

## 核心类型/函数
- **`PLUGIN_EVENT_PROTOCOL_VERSION: u32 = 1`** — 插件事件协议版本。任何**破坏性**变更（删除变体、改字段语义、改必填性）都必须自增，插件据 `envelope.version` 决定是否继续处理
- **`PluginEvent`** — 插件可见的事件枚举，`#[serde(tag = "type", rename_all = "snake_case", rename_all_fields = "camelCase")]`（即 JSON 形如 `{"type":"oj_switched","ojId":"…"}`）：
  - `OjSwitched { oj_id }`、`ContestSelected { contest_id }`
  - `AnnouncementChanged { contest_id, new_count }` — **只给数量，不给正文**（正文经插件 API 按权限查询）
  - `ProblemOpened { contest_id, problem_id }` — 不含题面内容
  - `SubmissionCreated { submission_id }`、`SubmissionJudged { submission_id, status }` — 状态摘要，不含耗时 / 内存 / 测试点明细
  - `WorkspaceSaved { workspace_id, revision, automatic }` — 不含文件内容，工作区路径也不暴露
  - `ConfigChanged`、`ThemeChanged`
  - `SessionLoggedIn { oj_id }`、`SessionLoggedOut { oj_id }`、`SessionExpired { oj_id }` — **只给 OJ 标识**，用户身份经插件 API 查询，避免协议层外泄内部标识
  - `ResyncRequired { lost }` — **协议级通知**：插件错过了 `lost` 条事件，必须重新查询当前状态（`lost = 0` 表示刚订阅、先同步一次）。由 `PluginHost` 在底层消费者 `Lagged` 时下发；插件收到后应调用插件 API 拉取真值，而不是假设事件会自动补发
  - **刻意不在白名单内**：`CoreEvent::TokenRotated` —— 凭证轮换是内部安全生命周期事件，对插件没有价值，暴露它只会扩大攻击面
- **`PluginEventEnvelope`** — `#[serde(rename_all = "camelCase")]` 的信封：`version: u32`（协议版本）、`sequence: u64`（**该插件订阅上的单调递增序号，从 1 开始** —— 插件可据此自行检测缺口，与 `ResyncRequired` 互为补充：前者插件侧兜底，后者宿主侧主动通知）、`occurred_at: i64`（UTC 毫秒时间戳）、`event: PluginEvent`

**禁止出现在本协议中的内容**：Token、密码、完整 Session、认证头、源代码、内部文件路径、内部缓存对象、未脱敏的用户实体、内部错误栈、内部实现细节。

## 直接依赖
- `serde::{Deserialize, Serialize}`

## 被依赖
- `plugin::api::mod`（通过 `pub mod event` 声明子模块）
- `plugin::host::event_adapter`（映射目标与信封构造）
- `plugin::host::plugin_host`（`PluginEventSink` 的投递对象）
- 未来运行时（v1.0 JS / v2.0 WASM）经 `PluginEventSink` 消费

## 逻辑流程
无（纯类型定义）。生产路径：`CoreEvent` →（`event_adapter::to_plugin_event` 白名单 + 裁剪）→ `PluginEvent` →（`wrap` 加 version / sequence / occurred_at）→ `PluginEventEnvelope` → `PluginEventSink::deliver`。

## 设计约束
- **为什么是独立类型而不是复用 `CoreEvent`**：① **稳定性** —— `CoreEvent` 是内部实现、随重构演进，插件协议必须能独立版本化；② **权限与脱敏** —— 协议层只暴露「插件确实需要」的字段，内部字段（用户 UUID、凭证轮换事实）不进协议；③ **可跨进程** —— `Serialize + Deserialize`，将来换 JS/WASM 运行时或跨进程投递时不需要改协议形状。

## 测试
`tests/event_tests.rs`：信封序列化为 camelCase 且事件带 `type` 标签（跨进程契约）、**协议载荷不含敏感字段**（对全部变体做字符串扫描）、`TokenRotated` 不属于协议（不存在对应变体）、协议版本被钉死（改动需显式自增）。

