use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, RwLock};

use tracing::{debug, warn};

use crate::core::event::app_event::AppEvent;
use crate::core::event::event_category::EventCategory;

/// 订阅 ID
pub type SubscriptionId = usize;

/// 事件处理回调类型
pub type EventHandler = Arc<dyn Fn(&AppEvent) + Send + Sync>;

/// 一个订阅项：回调 + 投递模式。
struct Subscriber {
    id: SubscriptionId,
    handler: EventHandler,
    /// `true` = 延迟投递（由后台线程执行，发布方不等待）
    deferred: bool,
}

/// 延迟投递任务（`Send` 的一次性闭包，持有事件副本与回调）。
type DeferredJob = Box<dyn FnOnce() + Send>;

/// 延迟投递队列：**专用后台线程 + 无界通道**消费。
///
/// 为什么不用 `tokio::spawn`：`publish` 会被同步上下文调用（如
/// `WorkspaceManager::save`、纯 `#[test]` 单元测试），那里没有 tokio runtime，
/// `tokio::spawn` 会 panic。专用线程与运行时无关，语义稳定且可测。
///
/// 线程随 `EventBus` 生命周期存在（`Drop` 时通道关闭、线程自然退出），
/// 不做 join —— 避免 handler 阻塞时卡住析构。
struct DeferredQueue {
    tx: Mutex<Sender<DeferredJob>>,
}

impl DeferredQueue {
    fn new() -> Self {
        let (tx, rx) = mpsc::channel::<DeferredJob>();
        let spawn_result = std::thread::Builder::new()
            .name("hinina-event-deferred".to_string())
            .spawn(move || {
                // 通道关闭（EventBus 析构）即退出
                while let Ok(job) = rx.recv() {
                    // panic 隔离：与同步投递同款，单个 handler 崩溃不终止消费线程
                    let result = std::panic::catch_unwind(AssertUnwindSafe(job));
                    if let Err(e) = result {
                        let msg = e
                            .downcast_ref::<&str>()
                            .copied()
                            .or_else(|| e.downcast_ref::<String>().map(|s| s.as_str()))
                            .unwrap_or("(unknown)");
                        warn!(panic_message = msg, "延迟事件 handler panic");
                    }
                }
            });

        if let Err(e) = spawn_result {
            // 线程创建失败属系统级异常：退化为「延迟订阅不投递」并在每次投递时告警，
            // 而不是 panic —— 事件是通知，不应让发布方因投递设施故障而失败
            warn!(error = %e, "延迟投递线程创建失败，延迟订阅将不生效");
        }

        Self {
            tx: Mutex::new(tx),
        }
    }

    /// 投递任务；队列已关闭（不可能，除非线程创建失败）时只告警。
    fn dispatch(&self, job: DeferredJob) {
        let tx = self.tx.lock().unwrap_or_else(|e| e.into_inner());
        if tx.send(job).is_err() {
            warn!("延迟事件投递失败：消费线程已退出");
        }
    }
}

/// EventBus — 应用事件总线。
///
/// 支持按 EventCategory 分类订阅，避免订阅者收到不关心的事件。
/// 订阅 EventCategory::All 将收到所有事件。
///
/// # 投递模式
///
/// - **同步投递**（[`EventBus::subscribe`]）：回调在 `publish` 的调用栈里执行，
///   适合纯内存处理（清缓存、置标志）。**不得在同步回调里做 I/O** ——
///   发布方的响应时间会随订阅者数量与各自开销增长。
/// - **延迟投递**（[`EventBus::subscribe_deferred`]）：回调由专用后台线程执行，
///   `publish` 立即返回，适合磁盘清理等 I/O 工作。
///
/// # 线程安全
///
/// - `subscribers` 用 RwLock 保护：发布时读锁（允许并发发布），订阅/取消时写锁
/// - 回调在锁外执行，避免回调中重新进入 EventBus 导致死锁
pub struct EventBus {
    subscribers: RwLock<HashMap<EventCategory, Vec<Subscriber>>>,
    next_id: Mutex<SubscriptionId>,
    deferred: DeferredQueue,
}

impl EventBus {
    /// 创建新的 EventBus
    #[must_use]
    pub fn new() -> Self {
        Self {
            subscribers: RwLock::new(HashMap::new()),
            next_id: Mutex::new(0),
            deferred: DeferredQueue::new(),
        }
    }

    /// 发布事件。分发到事件所属 category 和 EventCategory::All 的订阅者。
    ///
    /// 同步订阅者在锁外、本调用栈内执行；延迟订阅者被投递到后台线程后立即返回。
    /// 单个 handler panic 不影响其他 handler 执行。
    pub fn publish(&self, event: &AppEvent) {
        let category = event.category();

        // 在读锁内收集需要调用的 handler，然后在锁外执行
        let (sync_handlers, deferred_handlers) = {
            let subs = self.subscribers.read().unwrap_or_else(|e| e.into_inner());
            let mut sync_handlers = Vec::new();
            let mut deferred_handlers = Vec::new();

            for list in [
                subs.get(&category),
                subs.get(&EventCategory::All),
            ]
            .into_iter()
            .flatten()
            {
                for sub in list {
                    if sub.deferred {
                        deferred_handlers.push(Arc::clone(&sub.handler));
                    } else {
                        sync_handlers.push(Arc::clone(&sub.handler));
                    }
                }
            }

            (sync_handlers, deferred_handlers)
        };

        if sync_handlers.is_empty() && deferred_handlers.is_empty() {
            debug!(category = ?category, "事件发布，无订阅者");
            return;
        }

        debug!(
            category = ?category,
            sync_count = sync_handlers.len(),
            deferred_count = deferred_handlers.len(),
            "事件发布"
        );

        for handler in sync_handlers {
            // catch_unwind 隔离单个 handler panic，避免中断事件链
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                handler(event);
            }));
            if let Err(e) = result {
                let msg = e
                    .downcast_ref::<&str>()
                    .copied()
                    .or_else(|| e.downcast_ref::<String>().map(|s| s.as_str()))
                    .unwrap_or("(unknown)");
                warn!(category = ?category, panic_message = msg, "事件 handler panic");
            }
        }

        // 延迟投递：事件克隆进任务，发布方不等待订阅者的 I/O
        for handler in deferred_handlers {
            let event = event.clone();
            self.deferred
                .dispatch(Box::new(move || handler(&event)));
        }
    }

    /// 订阅指定类别的事件（**同步投递**），返回订阅 ID 用于取消订阅。
    ///
    /// 回调在 `publish` 的调用栈内执行 —— 只放纯内存处理，禁止 I/O。
    pub fn subscribe(
        &self,
        category: EventCategory,
        handler: EventHandler,
    ) -> SubscriptionId {
        self.add_subscriber(category, handler, false)
    }

    /// 订阅指定类别的事件（**延迟投递**），返回订阅 ID 用于取消订阅。
    ///
    /// 回调由专用后台线程执行，`publish` 立即返回 —— 用于磁盘清理等 I/O 工作。
    ///
    /// 使用前提：该 handler **不承担正确性职责**（正确性必须由同步路径或结构保证，
    /// 例如缓存键自带作用域，而不是依赖「清理及时」）。延迟意味着执行时机不确定，
    /// 也意味着进程退出时可能尚未执行。
    pub fn subscribe_deferred(
        &self,
        category: EventCategory,
        handler: EventHandler,
    ) -> SubscriptionId {
        self.add_subscriber(category, handler, true)
    }

    /// 注册订阅项（两种投递模式共用）。
    fn add_subscriber(
        &self,
        category: EventCategory,
        handler: EventHandler,
        deferred: bool,
    ) -> SubscriptionId {
        let id = {
            let mut next = self.next_id.lock().unwrap_or_else(|e| e.into_inner());
            let id = *next;
            *next = next.wrapping_add(1);
            id
        };

        let mut subs = self.subscribers.write().unwrap_or_else(|e| e.into_inner());
        subs.entry(category).or_default().push(Subscriber {
            id,
            handler,
            deferred,
        });

        debug!(
            subscription_id = id,
            category = ?category,
            deferred = deferred,
            "事件订阅"
        );
        id
    }

    /// 取消订阅。如果对应 ID 被取消则清理空类别。
    pub fn unsubscribe(&self, id: SubscriptionId) {
        let mut subs = self.subscribers.write().unwrap_or_else(|e| e.into_inner());
        // retain 掉所有类别中匹配的订阅
        for list in subs.values_mut() {
            list.retain(|sub| sub.id != id);
        }
        // 清理空类别以释放内存
        subs.retain(|_, list| !list.is_empty());

        debug!(subscription_id = id, "取消订阅");
    }

    /// 等待延迟队列排空（**仅测试用**）。
    ///
    /// 投递一个「哨兵任务」并等它执行完：队列 FIFO 且消费线程串行执行，
    /// 哨兵完成即意味着此前投递的任务都已执行完毕。测试用它避免
    /// 「断言早于延迟 handler 执行」造成的偶发失败（不是给生产代码用的同步点）。
    #[cfg(test)]
    pub(crate) fn flush_deferred(&self) {
        let (tx, rx) = mpsc::channel::<()>();
        self.deferred.dispatch(Box::new(move || {
            let _ = tx.send(());
        }));
        let _ = rx.recv_timeout(std::time::Duration::from_secs(5));
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
