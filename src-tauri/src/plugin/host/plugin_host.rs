//! `PluginHost` —— 插件事件适配、权限、脱敏与生命周期管理。
//!
//! # 定位（务必读）
//!
//! `PluginHost` **不是第二套业务总线**：
//!
//! - 它不创建自己的 channel，不维护自己的事件源；
//! - 它只从 [`CoreEventBus`] 取一条 receiver（经 `core::event::consumer::spawn_consumer`），
//!   与 Tauri 前端桥、审计消费者**平级**；
//! - 它把 `CoreEvent` 交给每个插件各自的 [`PluginEventAdapter`] 转换后投递。
//!
//! # 职责
//!
//! | 事项 | 落点 |
//! |---|---|
//! | 事件白名单、字段裁剪、脱敏、协议版本、序号 | `event_adapter` |
//! | 插件权限校验（须声明 `Notification`） | [`PluginHost::subscribe`] |
//! | 订阅生命周期（注册 / 注销） | [`PluginHost::subscribe`] / [`PluginHost::unsubscribe`] |
//! | 消费者落后后的重新同步通知 | `ResyncReason` → `PluginEvent::ResyncRequired` |
//! | 单个插件异常隔离 | 每个插件独立 sink，投递失败只影响该插件 |
//!
//! # 本次范围
//!
//! v0.x 只提供**架构边界**：没有 JS/WASM 运行时、没有插件目录扫描。
//! `PluginEventSink` 是运行时接入点（`mpsc::UnboundedSender` 已直接可用）。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use thiserror::Error;
use tracing::{debug, info, warn};

use crate::core::event::consumer::{spawn_consumer, ConsumerExit, ResyncReason};
use crate::core::event::core_event::CoreEvent;
use crate::core::event::core_event_bus::CoreEventBus;
use crate::plugin::api::event::PluginEventEnvelope;
use crate::plugin::host::event_adapter::PluginEventAdapter;
use crate::plugin::host::manifest::{PluginManifest, PluginPermission};

/// `PluginHost` 消费者的稳定名（日志字段）。
pub const PLUGIN_HOST_CONSUMER_NAME: &str = "plugin-host";

/// 插件事件投递出口。
///
/// 运行时（v1.0 JS / v2.0 WASM）只需实现本 trait 即可接入事件流；
/// [`tokio::sync::mpsc::UnboundedSender`] 已直接实现，供宿主把事件转交给插件线程。
///
/// 实现**不得** panic：投递失败应自行吞掉并记录（宿主也会隔离单个插件的异常）。
pub trait PluginEventSink: Send + Sync {
    /// 投递一条事件信封。实现应为非阻塞。
    fn deliver(&self, envelope: &PluginEventEnvelope);
}

impl PluginEventSink for tokio::sync::mpsc::UnboundedSender<PluginEventEnvelope> {
    fn deliver(&self, envelope: &PluginEventEnvelope) {
        // 通道关闭 = 插件已卸载或崩溃：只记录，绝不影响其他插件与宿主
        if self.send(envelope.clone()).is_err() {
            debug!("插件事件通道已关闭，本次投递被丢弃");
        }
    }
}

/// 插件宿主错误。
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PluginHostError {
    /// 插件 manifest 未声明 `Notification` 权限
    #[error("插件 `{plugin_id}` 未声明 Notification 权限，不能订阅事件")]
    PermissionDenied {
        /// 插件 id
        plugin_id: String,
    },
    /// 同一插件重复订阅
    #[error("插件 `{plugin_id}` 已有事件订阅")]
    AlreadySubscribed {
        /// 插件 id
        plugin_id: String,
    },
}

/// 单个插件的订阅项。
struct Subscription {
    /// 该插件专属的适配器（序号按订阅独立递增）
    adapter: PluginEventAdapter,
    sink: Arc<dyn PluginEventSink>,
}

/// 订阅表类型别名（抽出来是为了让消费者闭包只捕获**这张表**而不是整个宿主）。
type SubscriptionTable = Arc<Mutex<HashMap<String, Subscription>>>;

/// 插件宿主：管理插件的事件订阅与投递。
///
/// **不持有总线之外的任何事件设施**：`bus` 仅用于启动消费者（取一条 receiver）。
/// 消费者闭包捕获的是 [`SubscriptionTable`] 而不是 `Arc<PluginHost>` ——
/// 后者会形成「消费者 → 宿主 → 总线」的强引用环，让总线永不析构、
/// `ConsumerExit::Closed` 永不发生（进程退出时消费者只能被运行时强杀）。
pub struct PluginHost {
    bus: Arc<CoreEventBus>,
    /// 插件 id → 订阅。锁内只做「适配 + 收集」，**投递在锁外** ——
    /// 慢插件不得阻塞其他插件的投递，也不得阻塞新插件注册。
    subscriptions: SubscriptionTable,
}

impl PluginHost {
    /// 创建宿主（不订阅、不启动消费者）。
    #[must_use]
    pub fn new(bus: Arc<CoreEventBus>) -> Self {
        Self {
            bus,
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 注册插件的事件订阅。
    ///
    /// 权限判据是 manifest 声明了 [`PluginPermission::Notification`] ——
    /// 事件投递是「通知」能力，不能借 `ContestRead` 之类的读取权限顺带获得。
    ///
    /// 订阅建立即下发一条 `ResyncRequired { lost: 0 }`：晚于 `start()` 订阅的
    /// 插件收不到消费者的 `Startup` 通知，没有这条它会以为「没漏任何事件」。
    pub fn subscribe(
        &self,
        manifest: &PluginManifest,
        sink: Arc<dyn PluginEventSink>,
    ) -> Result<(), PluginHostError> {
        if !manifest
            .permissions
            .contains(&PluginPermission::Notification)
        {
            warn!(
                plugin_id = %manifest.id,
                "插件未声明 Notification 权限，拒绝事件订阅"
            );
            return Err(PluginHostError::PermissionDenied {
                plugin_id: manifest.id.clone(),
            });
        }

        let mut subs = self.subscriptions.lock().unwrap_or_else(|e| e.into_inner());
        if subs.contains_key(&manifest.id) {
            return Err(PluginHostError::AlreadySubscribed {
                plugin_id: manifest.id.clone(),
            });
        }
        let mut subscription = Subscription {
            adapter: PluginEventAdapter::new(),
            sink: Arc::clone(&sink),
        };
        // 在锁内投递是有意的（与 `dispatch_to_subscribers` 的「锁外投递」不同）：
        // 通知必须先于任何经消费者分发的事件到达 sink —— 否则插件会先看到
        // seq=2 的事件、再看到 seq=1 的通知（假缺口）。`PluginEventSink::deliver`
        // 契约上非阻塞，持锁投递这一次不会卡住其他插件。
        let notice = subscription.adapter.resync_notice(0);
        sink.deliver(&notice);
        subs.insert(manifest.id.clone(), subscription);
        info!(plugin_id = %manifest.id, "插件事件订阅已注册");
        Ok(())
    }

    /// 注销插件的事件订阅（幂等）。返回此前是否存在订阅。
    pub fn unsubscribe(&self, plugin_id: &str) -> bool {
        let removed = self
            .subscriptions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(plugin_id)
            .is_some();
        if removed {
            info!(plugin_id = %plugin_id, "插件事件订阅已注销");
        }
        removed
    }

    /// 当前订阅数（诊断 / 测试用）。
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.subscriptions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len()
    }

    /// 启动宿主消费者（从 `CoreEventBus` 取一条 receiver）。
    ///
    /// 与其他消费者共用同一条底层事件流；`Lagged` 时向所有插件下发
    /// `PluginEvent::ResyncRequired`（消费者启动时同样下发一次；晚于启动订阅的
    /// 插件由 [`PluginHost::subscribe`] 在订阅建立时补发，见其文档）。
    pub fn start(self: &Arc<Self>) -> tokio::task::JoinHandle<ConsumerExit> {
        // 只把订阅表交给消费者闭包（见 `PluginHost` 的引用环说明）
        let dispatch_subs = Arc::clone(&self.subscriptions);
        let resync_subs = Arc::clone(&self.subscriptions);

        spawn_consumer(
            &self.bus,
            PLUGIN_HOST_CONSUMER_NAME,
            move |event: CoreEvent| {
                let subs = Arc::clone(&dispatch_subs);
                async move {
                    dispatch_to_subscribers(&subs, &event);
                }
            },
            move |reason: ResyncReason| {
                let lost = match reason {
                    ResyncReason::Startup => 0,
                    ResyncReason::Lagged(lost) => lost,
                };
                notify_resync(&resync_subs, lost);
            },
        )
    }

    /// 把核心事件投递给所有已订阅插件（不经消费者）。
    ///
    /// 仅供测试直接驱动投递逻辑（生产路径一律经 `start()` 的消费者）。
    #[cfg(test)]
    fn dispatch(&self, event: &CoreEvent) {
        dispatch_to_subscribers(&self.subscriptions, event);
    }
}

/// 把核心事件投递给所有已订阅插件。
///
/// 锁内只做适配与收集（快），投递在锁外（慢插件不影响别人）。
fn dispatch_to_subscribers(subs: &SubscriptionTable, event: &CoreEvent) {
    let deliveries: Vec<(Arc<dyn PluginEventSink>, PluginEventEnvelope)> = {
        let mut subs = subs.lock().unwrap_or_else(|e| e.into_inner());
        subs.values_mut()
            .filter_map(|sub| {
                sub.adapter
                    .adapt(event)
                    .map(|envelope| (Arc::clone(&sub.sink), envelope))
            })
            .collect()
    };

    for (sink, envelope) in deliveries {
        sink.deliver(&envelope);
    }
}

/// 向所有插件下发「请重新同步」通知。
fn notify_resync(subs: &SubscriptionTable, lost: u64) {
    let notices: Vec<(Arc<dyn PluginEventSink>, PluginEventEnvelope)> = {
        let mut subs = subs.lock().unwrap_or_else(|e| e.into_inner());
        subs.values_mut()
            .map(|sub| (Arc::clone(&sub.sink), sub.adapter.resync_notice(lost)))
            .collect()
    };

    for (sink, envelope) in notices {
        sink.deliver(&envelope);
    }
}

#[cfg(test)]
#[path = "tests/plugin_host_tests.rs"]
mod tests;
