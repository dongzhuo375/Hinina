# core_event

## 职责
定义进程内**事实通知**的统一载荷 `CoreEvent`，以及事件名映射 `kind()`。

## 核心类型/函数
- **`CoreEvent`** — `Clone + Debug + PartialEq + Eq` 的轻量枚举，变体按领域分组：
  - 认证：`LoggedIn { oj_id, user_id }`、`LoggedOut { oj_id }`、`SessionExpired { oj_id }`、`TokenRotated { oj_id }`
  - 系统：`ConfigChanged`、`ThemeChanged`、`OjSwitched { oj_id }`
  - 比赛/题目：`ContestSelected { contest_id }`、`AnnouncementChanged { contest_id, new_ids }`、`ProblemOpened { contest_id, problem_id }`
  - 提交：`SubmissionCreated { submission_id }`、`SubmissionJudged { submission_id, status }`
  - 工作区：`WorkspaceSaved { workspace_id, revision, automatic }`
- **`CoreEvent::kind(&self) -> &'static str`** — 稳定短名（`auth.logged_in` / `workspace.saved` 等），用于结构化日志与插件事件名。不用分类枚举：单一 broadcast 通道下消费者直接 `match` 变体，分类表只会成为第二套需要同步维护的映射。

## 直接依赖
无外部依赖（纯枚举 + `kind()`；`CoreEvent` 本身**不实现 `Serialize`** —— 它不进 IPC、不进插件，插件侧另有一套 `PluginEvent` 协议）。

## 被依赖
- `core::event::core_event_bus`（通道载荷）
- `core::event::consumer`（消费者处理入参）
- 全部 Service（发布事实通知）与 `main.rs`（前端桥 / 审计 / 插件宿主）
- `plugin::host::event_adapter`（白名单映射的唯一来源）

## 逻辑流程
发布方在**核心动作完成后**调用 `CoreEventBus::publish(CoreEvent::…)`；消费者 `match` 变体决定行为。`kind()` 只在日志与插件协议命名处使用。

## 语义边界（硬约束）
- **只表示「已经发生」**，不表示「请执行某动作」。必须等待结果的动作不得依赖事件消费者。
- **允许丢失**：消费者必须能经 Service / IPC 查询重新同步；消费者**不能回滚**已完成的核心动作。
- **载荷只放 ID / 修订号 / 状态摘要**。禁止出现：Token、密码、完整 Session、认证头、源代码、内部文件路径、完整领域实体、内部缓存对象、未脱敏用户实体、内部错误栈。
- 大数据由消费者收到事件后经 Service / IPC 查询（事件只负责「提醒去看」）。

## 测试
`tests/core_event_tests.rs`：事件名唯一且稳定（手写期望值锁定契约）、载荷无敏感字段（对全部变体做 Debug 扫描，防止将来有人往变体里加 `token: String`）、可 Clone 且可比较、大载荷（文件内容/评测明细）缺席。
