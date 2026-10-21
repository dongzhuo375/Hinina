//! @plugin-module
//! @layer plugin/host
//! @version 1
//! @stability stable
//!
//! CoreEventBus 的单一插件消费者，仅向每插件有界队列非阻塞入队。
//! 不调用插件代码；队列是交付出口，不是第二套业务事件总线。
//! 队列满或关闭即注销，重新接入先 Resync。JS 执行与启停由 PluginManager 独立管理。

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::core::event::consumer::{spawn_consumer, ConsumerExit, ResyncReason};
use crate::core::event::core_event::CoreEvent;
use crate::core::event::core_event_bus::CoreEventBus;
use crate::plugin::api::event::PluginEventEnvelope;
use crate::plugin::host::event_adapter::PluginEventAdapter;
use crate::plugin::host::manifest::{PluginEventSubscription, PluginPermission};
use crate::plugin::permission::PluginGrant;

/// @plugin-api
/// @since 1.0
/// @stability stable
/// 统一消费者名。
pub const PLUGIN_HOST_CONSUMER_NAME: &str = "plugin-host";
/// @plugin-api
/// @since 1.0
/// @stability stable
/// 每插件队列容量，包括控制通知；慢消费者不能令队列无限增长。
pub const PLUGIN_EVENT_QUEUE_CAPACITY: usize = 128;

/// @plugin-api
/// @since 1.0
/// @stability stable
/// 宿主管理的队列出口，不接受用户 callback 或无界 sender。
/// 构造和投递仅限本模块，运行时只取得配套 Receiver。
pub struct PluginEventSink {
    sender: mpsc::Sender<PluginEventEnvelope>,
}

/// @plugin-api
/// @since 1.0
/// @stability stable
/// 宿主注册/启动错误，不包含内部状态或敏感数据。
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PluginHostError {
    #[error("plugin lacks events.subscribe grant")]
    PermissionDenied,
    #[error("plugin is already subscribed")]
    AlreadySubscribed,
    #[error("plugin host consumer is already started")]
    AlreadyStarted,
}

struct Subscription {
    version: String,
    adapter: PluginEventAdapter,
    allowed: HashSet<PluginEventSubscription>,
    sink: PluginEventSink,
}

type SubscriptionTable = Arc<Mutex<HashMap<String, Subscription>>>;

/// @plugin-api
/// @since 1.0
/// @stability stable
/// 仅管理订阅，不向插件暴露 CoreEvent/Receiver/Service。
/// 消费者只捕获订阅表，不捕获 Host 或 Bus 强引用。
pub struct PluginHost {
    bus: Arc<CoreEventBus>,
    subscriptions: SubscriptionTable,
    started: AtomicBool,
}

impl PluginHost {
    /// @plugin-api
    /// @since 1.0
    /// @stability stable
    /// 构造但不启动消费者、不加载插件。
    #[must_use]
    pub fn new(bus: Arc<CoreEventBus>) -> Self {
        Self {
            bus,
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
            started: AtomicBool::new(false),
        }
    }

    /// @plugin-api
    /// @since 1.0
    /// @permission events.subscribe
    /// @stability stable
    /// 原子注册并返回私有接收端；初始 seq=1 为 Resync。
    /// 未授权/未声明事件不耗序号，重复 ID 返回 AlreadySubscribed。
    pub fn subscribe(
        &self,
        grant: &PluginGrant,
    ) -> Result<mpsc::Receiver<PluginEventEnvelope>, PluginHostError> {
        self.subscribe_with_capacity(grant, PLUGIN_EVENT_QUEUE_CAPACITY)
    }

    fn subscribe_with_capacity(
        &self,
        grant: &PluginGrant,
        capacity: usize,
    ) -> Result<mpsc::Receiver<PluginEventEnvelope>, PluginHostError> {
        grant
            .require(PluginPermission::EventsSubscribe)
            .map_err(|_| PluginHostError::PermissionDenied)?;
        let manifest = grant.manifest();
        let mut subs = self.subscriptions.lock().unwrap_or_else(|e| e.into_inner());
        if subs.contains_key(&manifest.id) {
            return Err(PluginHostError::AlreadySubscribed);
        }
        let (sender, receiver) = mpsc::channel(capacity.max(1));
        let mut adapter = PluginEventAdapter::new();
        // 新队列容量至少 1，接收端仍在本栈中；初始 try_send 必定成功。
        let _ = sender.try_send(adapter.resync_notice(0));
        let allowed = grant.allowed_events().collect();
        subs.insert(
            manifest.id.clone(),
            Subscription {
                version: manifest.version.clone(),
                adapter,
                allowed,
                sink: PluginEventSink { sender },
            },
        );
        drop(subs);
        info!(plugin_id=%manifest.id, plugin_version=%manifest.version, runtime="js",
            event="resync.required", operation="subscribe", permission="events.subscribe", request_id="",
            "插件事件订阅已注册");
        Ok(receiver)
    }

    /// @plugin-api
    /// @since 1.0
    /// @stability stable
    /// 幂等注销。返回后不再入队；运行时停止时应丢弃已缓存消息。
    pub fn unsubscribe(&self, plugin_id: &str) -> bool {
        let removed = self
            .subscriptions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(plugin_id);
        if let Some(sub) = &removed {
            info!(plugin_id, plugin_version=%sub.version, runtime="js", event="", operation="unsubscribe",
                permission="events.subscribe", request_id="", "插件事件订阅已注销");
        }
        removed.is_some()
    }

    /// @plugin-api
    /// @since 1.0
    /// @stability stable
    /// 注册表大小；关闭的接收端最迟在下一次事件或控制通知时移除。
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.subscriptions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len()
    }

    /// @plugin-api
    /// @since 2.0
    /// @permission host-only
    /// @stability stable
    /// Success means a reconciliation prompt was enqueued, even after business events.
    pub fn request_resync(&self, id: &str) -> bool {
        let mut failures = Vec::new();
        let mut subs = self.subscriptions.lock().unwrap_or_else(|e| e.into_inner());
        let Some(sub) = subs.get_mut(id) else {
            return false;
        };
        let envelope = sub.adapter.resync_notice(0);
        if !enqueue(id, sub, envelope, &mut failures) {
            subs.remove(id);
            drop(subs);
            log_failures(failures);
            return false;
        }
        true
    }

    /// @plugin-api
    /// @since 1.0
    /// @stability stable
    /// 必须在 Tokio 上下文中调用，每个 Host 仅启动一次。
    /// 消费者重启由 spawn_consumer 监督；不加载代码或执行插件 handler。
    pub fn start(
        self: &Arc<Self>,
    ) -> Result<tokio::task::JoinHandle<ConsumerExit>, PluginHostError> {
        if self.started.swap(true, Ordering::SeqCst) {
            return Err(PluginHostError::AlreadyStarted);
        }
        let dispatch_subs = Arc::clone(&self.subscriptions);
        let resync_subs = Arc::clone(&self.subscriptions);
        Ok(spawn_consumer(
            &self.bus,
            PLUGIN_HOST_CONSUMER_NAME,
            move |event: CoreEvent| {
                let subs = Arc::clone(&dispatch_subs);
                async move {
                    dispatch_to_subscribers(&subs, &event);
                }
            },
            move |reason| {
                let lost = match reason {
                    ResyncReason::Startup => 0,
                    ResyncReason::Lagged(lost) => lost,
                };
                notify_resync(&resync_subs, lost);
            },
        ))
    }

    #[cfg(test)]
    pub(crate) fn dispatch(&self, event: &CoreEvent) {
        dispatch_to_subscribers(&self.subscriptions, event);
    }
}

// 临界区内只有宿主管理的 try_send，没有用户回调、await 或 I/O。
// 入队和移除共用此锁，保证序号按入队顺序且 unsubscribe 返回后不再投递。
fn dispatch_to_subscribers(subs: &SubscriptionTable, event: &CoreEvent) {
    let mut failures = Vec::new();
    subs.lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|id, sub| {
            if sub.sink.sender.is_closed() {
                return false;
            }
            if matches!(
                event,
                CoreEvent::OjSwitched { .. }
                    | CoreEvent::LoggedIn { .. }
                    | CoreEvent::LoggedOut { .. }
                    | CoreEvent::SessionExpired { .. }
                    | CoreEvent::ContestSelected { .. }
            ) {
                // Prompt reconciliation even when the plugin did not request the domain event.
                let notice = sub.adapter.resync_notice(0);
                if !enqueue(id, sub, notice, &mut failures) {
                    return false;
                }
            }
            let allowed = &sub.allowed;
            let envelope = sub
                .adapter
                .adapt_if(event, |e| allowed.contains(&e.subscription()));
            envelope.is_none_or(|envelope| enqueue(id, sub, envelope, &mut failures))
        });
    log_failures(failures);
}

fn notify_resync(subs: &SubscriptionTable, lost: u64) {
    let mut failures = Vec::new();
    subs.lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|id, sub| {
            let envelope = sub.adapter.resync_notice(lost);
            enqueue(id, sub, envelope, &mut failures)
        });
    log_failures(failures);
}

struct QueueFailure {
    id: String,
    version: String,
    event: &'static str,
    reason: &'static str,
}
fn log_failures(failures: Vec<QueueFailure>) {
    for failure in failures {
        warn!(plugin_id=?failure.id,plugin_version=%failure.version,runtime="js",event=failure.event,
            operation="queue.close",permission="events.subscribe",request_id="",reason=failure.reason,
            "插件队列不可用，注销订阅；重新接入必须 Resync");
    }
}
fn enqueue(
    id: &str,
    sub: &Subscription,
    envelope: PluginEventEnvelope,
    failures: &mut Vec<QueueFailure>,
) -> bool {
    let event = envelope.event.subscription();
    if let Err(error) = sub.sink.sender.try_send(envelope) {
        let reason = match error {
            mpsc::error::TrySendError::Full(_) => "full",
            mpsc::error::TrySendError::Closed(_) => "closed",
        };
        failures.push(QueueFailure {
            id: id.into(),
            version: sub.version.clone(),
            event: event.as_str(),
            reason,
        });
        return false;
    }
    true
}

#[cfg(test)]
#[path = "tests/plugin_host_tests.rs"]
mod tests;
