//! `CoreEventBus` —— 进程内事件总线的**唯一**底座。
//!
//! 建立在 [`tokio::sync::broadcast`] 之上，职责被刻意压到最小：
//!
//! - 创建 channel（统一容量配置）；
//! - 发布事件（同步非阻塞，`Sender::send` 不需要 `await`）；
//! - 创建 receiver；
//! - 统一的关闭 / 无消费者日志。
//!
//! **不负责**：订阅表、同步回调、延迟队列、自建分发器。这些能力已随旧
//! `EventBus`（`HashMap<EventCategory, Vec<Subscriber>>` + 专用延迟线程）一并删除 ——
//! 重新实现任何一项都会变成与 `broadcast` 等价的第二套分发系统。
//!
//! # 语义（务必读）
//!
//! ```text
//! broadcast::send() 成功
//!   ≠ 所有消费者已经处理完成
//!   ≠ 事件永久可靠送达
//!   ≠ 业务动作已经执行成功
//! ```
//!
//! 因此：
//! - **没有 receiver 时 `send` 返回 `Err`** —— 这不是业务失败，只记 `debug` 日志；
//! - 业务动作在发布**之前**就必须已经完成（发布失败不回滚业务动作）；
//! - 消费者落后时收到 `Lagged`，必须查询当前状态重新同步，不能假设会自动补发。

use tokio::sync::broadcast;
use tracing::debug;

use crate::core::event::core_event::CoreEvent;

/// broadcast 通道容量（每个 receiver 的待消费上限）。
///
/// 取值权衡：事件是轻量事实通知，正常消费远快于产生；容量只需覆盖
/// 「消费者短暂卡顿」的窗口。过大只会把内存占用与「消费者已严重落后」
/// 的发现时间一起推后，故不追求更大。
pub const CORE_EVENT_CHANNEL_CAPACITY: usize = 1024;

/// 进程内核心事件总线。
///
/// `Clone` 不需要（用 `Arc<CoreEventBus>` 共享）：多份 sender 会让
/// 「谁在发布事件」变得不可推理。
#[derive(Debug)]
pub struct CoreEventBus {
    tx: broadcast::Sender<CoreEvent>,
}

impl CoreEventBus {
    /// 按默认容量创建。
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(CORE_EVENT_CHANNEL_CAPACITY)
    }

    /// 按指定容量创建（容量为 0 时钳到 1 —— `broadcast` 不接受 0）。
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity.max(1));
        // 初始 receiver 立即丢弃：订阅一律由消费者显式 `subscribe()`，
        // 避免「总线自己持有一个永不消费的 receiver」把 `send` 的语义搞糊。
        Self { tx }
    }

    /// 发布事件（同步非阻塞）。
    ///
    /// 无消费者时只记 `debug`：事件是事实通知，没有观察者不代表业务失败，
    /// 更**不能**让已经完成的核心动作因此回滚或报错。
    pub fn publish(&self, event: CoreEvent) {
        // 事件名必须在 `send` 之前取：`send` 会消费掉事件，
        // 而成功分支不会把事件还回来（`SendError` 才会）。
        let kind = event.kind();
        match self.tx.send(event) {
            Ok(receivers) => debug!(event = kind, receivers, "核心事件已发布"),
            Err(_) => debug!(event = kind, "核心事件发布时无消费者（事实通知允许丢失）"),
        }
    }

    /// 创建一个新的 receiver。
    ///
    /// receiver 只收到**订阅之后**发布的事件：消费者启动时必须先自行查询
    /// 一次当前状态（`consumer::spawn_consumer` 的 `resync` 回调即为此）。
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<CoreEvent> {
        self.tx.subscribe()
    }

    /// 当前 receiver 数量（诊断 / 测试用）。
    #[must_use]
    pub fn receiver_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl Default for CoreEventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "tests/core_event_bus_tests.rs"]
mod tests;
