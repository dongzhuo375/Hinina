use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
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
    // 订阅建立即下发一条 ResyncRequired（晚于 start() 订阅的插件靠它知道要先同步）
    assert_eq!(sink.events(), vec![PluginEvent::ResyncRequired { lost: 0 }]);

    host.dispatch(&CoreEvent::ConfigChanged);
    assert_eq!(sink.count(), 2);

    assert!(host.unsubscribe("p1"));
    assert!(!host.unsubscribe("p1"), "重复注销应返回 false");

    host.dispatch(&CoreEvent::ThemeChanged);
    assert_eq!(sink.count(), 2, "注销后不应再收到事件");
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

    // 首条是订阅通知，其后只有白名单内的 ConfigChanged（TokenRotated 被过滤）
    assert_eq!(
        sink.events(),
        vec![
            PluginEvent::ResyncRequired { lost: 0 },
            PluginEvent::ConfigChanged
        ]
    );
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

    // seq=1 是订阅通知，其后是三条事件：序号连续、无跳号
    assert_eq!(a.sequences(), vec![1, 2, 3, 4]);
    assert_eq!(b.sequences(), vec![1, 2, 3, 4]);
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

    // 健康插件：订阅通知 + 两条事件
    assert_eq!(healthy.count(), 3, "健康插件不应受投递失败的插件影响");
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
    // 订阅建立时宿主先下发一条 ResyncRequired（提示插件先同步一次）
    assert_eq!(sink.count(), 1);
    assert_eq!(sink.events()[0], PluginEvent::ResyncRequired { lost: 0 });

    let task = host.start();
    // 消费者启动的 Startup resync 再下发一次（先于 start() 订阅的插件两条都收）
    wait_until(|| sink.count() >= 2, "消费者启动后应再次下发同步通知").await;

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

/// 晚于 `start()` 订阅的插件同样收到初始 `ResyncRequired`：
/// 消费者的 Startup 通知只发一次，后订阅的插件只能靠订阅建立时的补发
/// 知道「先同步一次当前状态」。
#[tokio::test]
async fn subscribe_after_start_receives_initial_notice() {
    let (host, bus) = host_with_bus();

    // 金丝雀插件先行订阅并等消费者完成 Startup resync（收到第 2 条通知），
    // 确保「晚订阅」的场景是确定性的 —— 否则 Startup 通知可能晚于订阅到达
    let canary = Arc::new(RecordingSink::default());
    host.subscribe(
        &manifest("canary", vec![PluginPermission::Notification]),
        Arc::clone(&canary) as Arc<dyn PluginEventSink>,
    )
    .unwrap();
    let task = host.start();
    wait_until(|| canary.count() >= 2, "消费者启动完成").await;

    let sink = Arc::new(RecordingSink::default());
    host.subscribe(
        &manifest("late-comer", vec![PluginPermission::Notification]),
        Arc::clone(&sink) as Arc<dyn PluginEventSink>,
    )
    .unwrap();

    // 订阅即收到通知（同步投递，无需等待）
    assert_eq!(sink.events(), vec![PluginEvent::ResyncRequired { lost: 0 }]);
    assert_eq!(sink.sequences(), vec![1], "通知占用 seq=1，后续事件从 2 起");

    // 之后发布的事件正常送达，且序号接在通知之后（无假缺口）
    bus.publish(CoreEvent::ConfigChanged);
    wait_until(|| sink.count() >= 2, "订阅后发布的事件应送达").await;
    assert_eq!(sink.sequences(), vec![1, 2]);

    drop(host);
    drop(bus);
    assert_eq!(task.await.unwrap(), ConsumerExit::Closed);
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

    // 先启动消费者并让它完成启动通知（订阅通知 1 条 + 启动通知 1 条），再制造落后
    let task = host.start();
    wait_until(|| sink.count() >= 2, "启动通知").await;

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

/// sink 的 `deliver` 内回调宿主方法（此处 `subscriber_count`，取同一把
/// 不可重入锁）不得死锁：宿主必须在**无锁上下文**调用用户实现。
/// 死锁表现为订阅线程挂住 —— 用带超时的信道把「挂住」转成测试失败。
#[test]
fn reentrant_sink_does_not_deadlock() {
    struct ReentrantSink {
        host: Weak<PluginHost>,
        reentered: Arc<AtomicUsize>,
    }
    impl PluginEventSink for ReentrantSink {
        fn deliver(&self, _envelope: &PluginEventEnvelope) {
            if let Some(host) = self.host.upgrade() {
                self.reentered.fetch_add(1, Ordering::SeqCst);
                // 修复前：subscribe 持锁调用 deliver → 此处对同一把锁再次
                // lock() → 同线程自死锁
                let _ = host.subscriber_count();
            }
        }
    }

    let (host, _bus) = host_with_bus();
    let reentered = Arc::new(AtomicUsize::new(0));
    let sink = Arc::new(ReentrantSink {
        host: Arc::downgrade(&host),
        reentered: Arc::clone(&reentered),
    });

    let (tx, rx) = std::sync::mpsc::channel();
    let subscriber = Arc::clone(&host);
    std::thread::spawn(move || {
        let ok = subscriber
            .subscribe(
                &manifest("reentrant", vec![PluginPermission::Notification]),
                sink,
            )
            .is_ok();
        let _ = tx.send(ok);
    });

    let ok = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("subscribe 应在超时前完成（死锁会挂住订阅线程）");
    assert!(ok, "订阅应成功");
    assert_eq!(
        reentered.load(Ordering::SeqCst),
        1,
        "初始通知应触发一次重入回调"
    );
    assert_eq!(host.subscriber_count(), 1);
}
