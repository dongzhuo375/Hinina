//! @plugin-module
//! @layer plugin/host
//! @version 1
//! @stability stable
//!
//! CoreEventBus ??????????????????????????
//! ??????????????????????????????
//! ?/??????????? Resync??????JS ??? Manager ????

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
/// ???????
pub const PLUGIN_HOST_CONSUMER_NAME: &str = "plugin-host";
/// @plugin-api
/// @since 1.0
/// @stability stable
/// ?????????????????????????????
pub const PLUGIN_EVENT_QUEUE_CAPACITY: usize = 128;

/// @plugin-api
/// @since 1.0
/// @stability stable
/// ?????????????????? callback ?????? sender?
/// ??????????????????? Receiver?
pub struct PluginEventSink {
    sender: mpsc::Sender<PluginEventEnvelope>,
}

/// @plugin-api
/// @since 1.0
/// @stability stable
/// ????/??????????????????
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
/// ???????????? CoreEvent/Receiver/Service?
/// ????????????? Host ? Bus ?????
pub struct PluginHost {
    bus: Arc<CoreEventBus>,
    subscriptions: SubscriptionTable,
    started: AtomicBool,
}

impl PluginHost {
    /// @plugin-api
    /// @since 1.0
    /// @stability stable
    /// ????????????????
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
    /// ????????????????? seq=1 ? Resync?????????
    /// ???????????? ID ?? AlreadySubscribed???????????
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
        // ????????? 1??????????try_send ?????
        let _ = sender.try_send(adapter.resync_notice(0));
        let allowed = manifest
            .events
            .iter()
            .copied()
            .filter(|event| {
                event
                    .required_permission()
                    .is_none_or(|p| grant.require(p).is_ok())
            })
            .collect();
        subs.insert(
            manifest.id.clone(),
            Subscription {
                version: manifest.version.clone(),
                adapter,
                allowed,
                sink: PluginEventSink { sender },
            },
        );
        info!(plugin_id=%manifest.id, plugin_version=%manifest.version, runtime="js",
            event="resync.required", operation="subscribe", permission="events.subscribe", request_id="",
            "?????????");
        Ok(receiver)
    }

    /// @plugin-api
    /// @since 1.0
    /// @stability stable
    /// ?????????????????????????????????
    pub fn unsubscribe(&self, plugin_id: &str) -> bool {
        let removed = self
            .subscriptions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(plugin_id);
        if let Some(sub) = &removed {
            info!(plugin_id, plugin_version=%sub.version, runtime="js", event="", operation="unsubscribe",
                permission="events.subscribe", request_id="", "?????????");
        }
        removed.is_some()
    }

    /// @plugin-api
    /// @since 1.0
    /// @stability stable
    /// ?????????????????????????????
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.subscriptions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len()
    }

    /// @plugin-api
    /// @since 1.0
    /// @stability stable
    /// ??? Tokio ????????? Host ??????????????
    /// spawn_consumer ????????????????? handler?
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
    fn dispatch(&self, event: &CoreEvent) {
        dispatch_to_subscribers(&self.subscriptions, event);
    }
}

// ??????????? try_send????????await ? I/O????????
// ??????? unsubscribe ??????????????????????
fn dispatch_to_subscribers(subs: &SubscriptionTable, event: &CoreEvent) {
    subs.lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|id, sub| {
            if sub.sink.sender.is_closed() {
                return false;
            }
            let allowed = &sub.allowed;
            let envelope = sub
                .adapter
                .adapt_if(event, |e| allowed.contains(&e.subscription()));
            envelope.is_none_or(|envelope| enqueue(id, sub, envelope))
        });
}

fn notify_resync(subs: &SubscriptionTable, lost: u64) {
    subs.lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|id, sub| {
            let envelope = sub.adapter.resync_notice(lost);
            enqueue(id, sub, envelope)
        });
}

fn enqueue(id: &str, sub: &Subscription, envelope: PluginEventEnvelope) -> bool {
    let event = envelope.event.subscription();
    if let Err(error) = sub.sink.sender.try_send(envelope) {
        let reason = match error {
            mpsc::error::TrySendError::Full(_) => "full",
            mpsc::error::TrySendError::Closed(_) => "closed",
        };
        warn!(plugin_id=id, plugin_version=%sub.version, runtime="js", event=?event,
            operation="queue.close", permission="events.subscribe", request_id="", reason,
            "??????????????????? Resync");
        return false;
    }
    true
}

#[cfg(test)]
#[path = "tests/plugin_host_tests.rs"]
mod tests;
