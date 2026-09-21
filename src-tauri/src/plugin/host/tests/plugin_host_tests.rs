use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::core::event::core_event_bus::CoreEventBus;
use crate::plugin::api::event::PluginEvent;
use crate::plugin::host::extension::ExtensionPoint;
use crate::plugin::host::manifest::PluginManifest;

/// 记录型 sink：把投递到的事件攒起来供断言。
#[derive(Default)]
struct RecordingSink {
    envelopes: Mutex<Vec<PluginEventEnvelope>>,
}

impl RecordingSink {
    fn events(&self) -> Vec<PluginEvent> {
        self.envelopes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|envelope| envelope.event.clone())
            .collect()
    }

    fn sequences(&self) -> Vec<u64> {
        self.envelopes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|envelope| envelope.sequence)
            .collect()
    }

    fn count(&self) -> usize {
        self.envelopes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len()
    }
}

impl PluginEventSink for RecordingSink {
    fn deliver(&self, envelope: &PluginEventEnvelope) {
        self.envelopes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(envelope.clone());
    }
}

/// 测试用 manifest。
fn manifest(id: &str, permissions: Vec<PluginPermission>) -> PluginManifest {
    PluginManifest {
        id: id.to_string(),
        name: id.to_string(),
        version: "1.0.0".to_string(),
        description: String::new(),
        author: String::new(),
        permissions,
        extension_points: Vec::<ExtensionPoint>::new(),
        min_app_version: "0.1.0".to_string(),
    }
}

fn host_with_bus() -> (Arc<PluginHost>, Arc<CoreEventBus>) {
    let bus = Arc::new(CoreEventBus::new());
    (Arc::new(PluginHost::new(Arc::clone(&bus))), bus)
}

async fn wait_until(mut condition: impl FnMut() -> bool, what: &str) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if condition() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("等待超时：{what}");
}

/// 未声明 `Notification` 权限 → 拒绝订阅（不能借读取权限顺带获得通知能力）。
#[test]
fn subscribe_requires_notification_permission() {
    let (host, _bus) = host_with_bus();
    let sink = Arc::new(RecordingSink::default());

    let err = host
        .subscribe(&manifest("p1", vec![PluginPermission::ContestRead]), sink)
        .unwrap_err();
    assert_eq!(
        err,
        PluginHostError::PermissionDenied {
            plugin_id: "p1".into()
        }
    );
    assert_eq!(host.subscriber_count(), 0);
}

/// 同一插件重复订阅被拒绝（避免同一插件收到两份事件、序号错乱）。
#[test]
fn duplicate_subscription_is_rejected() {
    let (host, _bus) = host_with_bus();
    let perms = || vec![PluginPermission::Notification];

    host.subscribe(&manifest("p1", perms()), Arc::new(RecordingSink::default()))
        .unwrap();
    let err = host
        .subscribe(&manifest("p1", perms()), Arc::new(RecordingSink::default()))
        .unwrap_err();
    assert_eq!(
        err,
        PluginHostError::AlreadySubscribed {
            plugin_id: "p1".into()
        }
    );
    assert_eq!(host.subscriber_count(), 1);
}

/// 注销幂等，且注销后不再投递。
#[test]
fn unsubscribe_is_idempotent_and_stops_delivery() {
    let (host, _bus) = host_with_bus();
    let sink = Arc::new(RecordingSink::default());

    host.subscribe(
        &manifest("p1", vec![PluginPermission::Notification]),
        Arc::clone(&sink) as Arc<dyn PluginEventSink>,
    )
    .unwrap();

    host.dispatch(&CoreEvent::ConfigChanged);
    assert_eq!(sink.count(), 1);

    assert!(host.unsubscribe("p1"));
    assert!(!host.unsubscribe("p1"), "重复注销应返回 false");

    host.dispatch(&CoreEvent::ThemeChanged);
    assert_eq!(sink.count(), 1, "注销后不应再收到事件");
}

/// 白名单过滤在宿主层同样生效：`TokenRotated` 不投递给任何插件。
#[test]
fn host_filters_non_whitelisted_events() {
    let (host, _bus) = host_with_bus();
    let sink = Arc::new(RecordingSink::default());
    host.subscribe(
        &manifest("p1", vec![PluginPermission::Notification]),
        Arc::clone(&sink) as Arc<dyn PluginEventSink>,
    )
    .unwrap();

    host.dispatch(&CoreEvent::TokenRotated {
        oj_id: "HOJ".into(),
    });
    host.dispatch(&CoreEvent::ConfigChanged);

    assert_eq!(sink.events(), vec![PluginEvent::ConfigChanged]);
}

/// 每个插件的序号独立且连续（一个插件的订阅不影响另一个）。
#[test]
fn sequences_are_per_subscription() {
    let (host, _bus) = host_with_bus();
    let a = Arc::new(RecordingSink::default());
    let b = Arc::new(RecordingSink::default());

    host.subscribe(
        &manifest("p-a", vec![PluginPermission::Notification]),
        Arc::clone(&a) as Arc<dyn PluginEventSink>,
    )
    .unwrap();
    host.subscribe(
        &manifest("p-b", vec![PluginPermission::Notification]),
        Arc::clone(&b) as Arc<dyn PluginEventSink>,
    )
    .unwrap();

    host.dispatch(&CoreEvent::ConfigChanged);
    host.dispatch(&CoreEvent::ThemeChanged);
    host.dispatch(&CoreEvent::OjSwitched {
        oj_id: "HOJ".into(),
    });

    assert_eq!(a.sequences(), vec![1, 2, 3]);
    assert_eq!(b.sequences(), vec![1, 2, 3]);
}

/// 单个插件的 sink 已关闭 → 不影响其他插件继续收到事件。
#[test]
fn failing_sink_does_not_affect_other_plugins() {
    let (host, _bus) = host_with_bus();
    let healthy = Arc::new(RecordingSink::default());

    // 已关闭的 mpsc 通道：投递必然失败
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<PluginEventEnvelope>();
    drop(rx);

    host.subscribe(
        &manifest("p-broken", vec![PluginPermission::Notification]),
        Arc::new(tx) as Arc<dyn PluginEventSink>,
    )
    .unwrap();
    host.subscribe(
        &manifest("p-healthy", vec![PluginPermission::Notification]),
        Arc::clone(&healthy) as Arc<dyn PluginEventSink>,
    )
    .unwrap();

    host.dispatch(&CoreEvent::ConfigChanged);
    host.dispatch(&CoreEvent::ThemeChanged);

    assert_eq!(healthy.count(), 2, "健康插件不应受投递失败的插件影响");
}

/// 端到端：宿主消费者挂在同一条 `CoreEventBus` 上，
/// 事件经 broadcast → 适配 → 投递到插件 sink。
#[tokio::test]
async fn host_consumer_delivers_from_bus() {
    let (host, bus) = host_with_bus();
    let sink = Arc::new(RecordingSink::default());
    host.subscribe(
        &manifest("p1", vec![PluginPermission::Notification]),
        Arc::clone(&sink) as Arc<dyn PluginEventSink>,
    )
    .unwrap();

    let task = host.start();
    // 订阅建立时宿主会先下发一条 ResyncRequired（提示插件先同步一次）
    wait_until(|| sink.count() >= 1, "订阅建立后应收到重新同步通知").await;
    assert_eq!(sink.events()[0], PluginEvent::ResyncRequired { lost: 0 });

    bus.publish(CoreEvent::SubmissionJudged {
        submission_id: "s-1".into(),
        status: "Accepted".into(),
    });

    wait_until(
        || {
            sink.events().contains(&PluginEvent::SubmissionJudged {
                submission_id: "s-1".into(),
                status: "Accepted".into(),
            })
        },
        "插件应收到评测完成事件",
    )
    .await;

    // 宿主与总线都释放后，消费者应收到 Closed 并干净退出
    // （消费者闭包只捕获订阅表，不形成「消费者 → 宿主 → 总线」的强引用环）
    drop(host);
    drop(bus);
    let exit = task.await.expect("宿主消费者不应 panic");
    assert_eq!(exit, ConsumerExit::Closed);
}

/// 宿主消费者落后（`Lagged`）→ 向所有插件下发 `ResyncRequired { lost }`。
#[tokio::test]
async fn lagged_notifies_plugins_to_resync() {
    let bus = Arc::new(CoreEventBus::with_capacity(1));
    let host = Arc::new(PluginHost::new(Arc::clone(&bus)));
    let sink = Arc::new(RecordingSink::default());
    host.subscribe(
        &manifest("p1", vec![PluginPermission::Notification]),
        Arc::clone(&sink) as Arc<dyn PluginEventSink>,
    )
    .unwrap();

    // 先启动消费者并让它完成启动通知，再制造落后
    let task = host.start();
    wait_until(|| sink.count() >= 1, "启动通知").await;

    // 塞满容量后立刻密集发布：消费者来不及消费 → Lagged
    for _ in 0..50 {
        bus.publish(CoreEvent::ThemeChanged);
    }

    wait_until(
        || {
            sink.events()
                .iter()
                .any(|event| matches!(event, PluginEvent::ResyncRequired { lost } if *lost > 0))
        },
        "落后后应下发带丢失数量的重新同步通知",
    )
    .await;

    drop(host);
    drop(bus);
    assert_eq!(task.await.unwrap(), ConsumerExit::Closed);
}

/// `PluginHost` 不创建第二套总线：它只持有 `CoreEventBus` 的 `Arc`，
/// 且自身没有对外暴露的发布接口（编译期即保证 —— 本用例锁定意图）。
#[test]
fn host_does_not_own_a_second_bus() {
    let (host, bus) = host_with_bus();
    assert_eq!(bus.receiver_count(), 0, "创建宿主不应自行订阅");

    let _task_ready = host.subscriber_count();
    assert_eq!(host.subscriber_count(), 0);
    // 宿主上没有 `publish` / `emit` 之类的入口：插件只能消费，不能生产核心事件
}
