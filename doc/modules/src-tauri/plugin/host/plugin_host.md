# host/plugin_host

## 职责
唯一 CoreEventBus 插件消费者：只负责白名单/权限过滤、安全 PluginEventAdapter 和有界投递。不运行 JS、不加载文件、不管理业务动作。

## 核心类型/函数
- `PLUGIN_HOST_CONSUMER_NAME = "plugin-host"` — core 消费者注册名（Startup/Lagged 沿用 core 统一消费者）
- `PLUGIN_EVENT_QUEUE_CAPACITY = 128` — 每插件 mpsc 队列容量
- **`PluginHost`** — 订阅表与投递编排；`subscribe_with_capacity(...)`（`subscribe(&PluginGrant)` 需 events.subscribe，重复 ID 拒绝）、`request_resync(id)`、注销
- `PluginEventSink` — 单插件投递端
- `PluginHostError` — 稳定错误

## 直接依赖
- `core::event::core_event`（唯一事件源）
- `plugin::api::event`（信封）、`plugin::host::event_adapter`、`plugin::permission::PluginGrant`
- `tokio::sync::mpsc`、`tracing`、`thiserror`

## 被依赖
- `plugin::manager`（订阅生命周期与 Resync 触发）

## 逻辑流程
- **投递**：初始 Resync 先于其他投递；过滤后分配 sequence。队列满/关闭即注销单插件，worker 发现关闭后停止并撤销调用；注销后放弃已缓冲消息，重启先 Resync
- **request_resync**：始终尝试把 Resync 入队（即使队列已有业务事件），返回 true 代表提示实际入队；满/关闭时注销该订阅并返回 false——不能因队列非空就静默跳过仍报成功；容量 128 继续约束积压。账号/OJ/比赛变化的事实通知可附加 Resync，即使该领域事件未声明
- **语义**：`lost=0` 不代表完整历史；没有第二套业务事件总线。单插件故障不阻止其他插件或核心。Manager 负责独立 worker、配置授权和启停（见 [Runtime](../runtime/mod.md)）
- **锁纪律**：订阅和队列失败只在锁内收集诊断，释放 subscriptions 锁后才调用 tracing——慢日志 writer 不占用全体订阅锁
