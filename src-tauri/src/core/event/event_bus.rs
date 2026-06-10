use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use crate::core::event::app_event::AppEvent;
use crate::core::event::event_category::EventCategory;

/// 订阅 ID
pub type SubscriptionId = usize;

/// 事件处理回调类型
pub type EventHandler = Arc<dyn Fn(AppEvent) + Send + Sync>;

/// EventBus — 应用事件总线。
///
/// 支持按 EventCategory 分类订阅，避免订阅者收到不关心的事件。
pub struct EventBus {
    subscribers: RwLock<HashMap<EventCategory, Vec<(SubscriptionId, EventHandler)>>>,
    next_id: Mutex<SubscriptionId>,
}

impl EventBus {
    /// 创建新的 EventBus
    #[must_use]
    pub fn new() -> Self {
        Self {
            subscribers: RwLock::new(HashMap::new()),
            next_id: Mutex::new(0),
        }
    }

    /// 发布事件。对所有订阅了匹配类别的 handler 触发回调。
    /// 同时也通知订阅了 EventCategory::All 的 handler。
    pub fn publish(&self, event: AppEvent) {
        // TODO: 实现事件分发逻辑
        let _ = event;
        todo!("EventBus::publish()")
    }

    /// 订阅指定类别的事件，返回订阅 ID 用于取消订阅
    pub fn subscribe(
        &self,
        category: EventCategory,
        handler: EventHandler,
    ) -> SubscriptionId {
        // TODO: 实现订阅逻辑
        let _ = (category, handler);
        todo!("EventBus::subscribe()")
    }

    /// 取消订阅
    pub fn unsubscribe(&self, id: SubscriptionId) {
        // TODO: 实现取消订阅逻辑
        let _ = id;
        todo!("EventBus::unsubscribe()")
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
