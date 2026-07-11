use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use tracing::debug;

use crate::core::event::app_event::AppEvent;
use crate::core::event::event_category::EventCategory;

/// 订阅 ID
pub type SubscriptionId = usize;

/// 事件处理回调类型
pub type EventHandler = Arc<dyn Fn(&AppEvent) + Send + Sync>;

/// EventBus — 应用事件总线。
///
/// 支持按 EventCategory 分类订阅，避免订阅者收到不关心的事件。
/// 订阅 EventCategory::All 将收到所有事件。
///
/// # 线程安全
///
/// - `subscribers` 用 RwLock 保护：发布时读锁（允许并发发布），订阅/取消时写锁
/// - 回调在锁外执行，避免回调中重新进入 EventBus 导致死锁
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

    /// 发布事件。分发到事件所属 category 和 EventCategory::All 的订阅者。
    ///
    /// 回调在锁外执行，避免回调中操作 EventBus 导致死锁。
    pub fn publish(&self, event: AppEvent) {
        let category = event.category();
        let event_arc = Arc::new(event);

        // 在读锁内收集需要调用的 handler，然后在锁外执行
        let handlers_to_call = {
            let subs = self.subscribers.read().unwrap_or_else(|e| e.into_inner());
            let mut handlers = Vec::new();

            if let Some(list) = subs.get(&category) {
                for (_id, handler) in list {
                    handlers.push(Arc::clone(handler));
                }
            }
            if let Some(list) = subs.get(&EventCategory::All) {
                for (_id, handler) in list {
                    handlers.push(Arc::clone(handler));
                }
            }

            handlers
        };

        if handlers_to_call.is_empty() {
            debug!(category = ?category, "事件发布，无订阅者");
            return;
        }

        debug!(
            category = ?category,
            handler_count = handlers_to_call.len(),
            "事件发布"
        );

        for handler in handlers_to_call {
            handler(&event_arc);
        }
    }

    /// 订阅指定类别的事件，返回订阅 ID 用于取消订阅。
    pub fn subscribe(
        &self,
        category: EventCategory,
        handler: EventHandler,
    ) -> SubscriptionId {
        let id = {
            let mut next = self.next_id.lock().unwrap_or_else(|e| e.into_inner());
            let id = *next;
            *next = next.wrapping_add(1);
            id
        };

        let mut subs = self.subscribers.write().unwrap_or_else(|e| e.into_inner());
        subs.entry(category).or_default().push((id, handler));

        debug!(
            subscription_id = id,
            category = ?category,
            "事件订阅"
        );
        id
    }

    /// 取消订阅。如果对应 ID 被取消则清理空类别。
    pub fn unsubscribe(&self, id: SubscriptionId) {
        let mut subs = self.subscribers.write().unwrap_or_else(|e| e.into_inner());
        // retain 掉所有类别中匹配的订阅
        for list in subs.values_mut() {
            list.retain(|(sid, _)| *sid != id);
        }
        // 清理空类别以释放内存
        subs.retain(|_, list| !list.is_empty());

        debug!(subscription_id = id, "取消订阅");
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "tests/event_bus_tests.rs"]
mod tests;
