//! 事件模块 —— 进程内事实通知的底座。
//!
//! 分层：
//! - [`core_event`]：`CoreEvent`，统一的事件载荷（只放 ID / 修订号 / 状态摘要）
//! - [`core_event_bus`]：`CoreEventBus`，`tokio::sync::broadcast` 的极薄封装
//! - [`consumer`]：`spawn_consumer`，受监督的消费者循环（`Lagged` / `Closed` / panic）
//!
//! 旧实现（`AppEvent` + `EventCategory` + 手写 `EventBus` 的同步回调表与延迟线程）
//! 已删除：它同时承担了「事实通知」与「必须完成的核心动作」两种语义，
//! 导致 Token 轮换、OJ 切换、配置热生效三处出现隐式同步依赖。

pub mod consumer;
pub mod core_event;
pub mod core_event_bus;
