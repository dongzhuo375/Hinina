use std::sync::Arc;

use super::*;

/// 多个消费者都能收到同一事件（`broadcast` 的扇出语义）。
#[tokio::test]
async fn all_receivers_get_the_event() {
    let bus = CoreEventBus::new();
    let mut rx1 = bus.subscribe();
    let mut rx2 = bus.subscribe();

    bus.publish(CoreEvent::ConfigChanged);

    assert_eq!(rx1.recv().await.unwrap(), CoreEvent::ConfigChanged);
    assert_eq!(rx2.recv().await.unwrap(), CoreEvent::ConfigChanged);
}

/// 没有消费者时发布**不得** panic、不得报错：核心业务动作已完成，
/// 通知没有观察者不代表业务失败。
#[test]
fn publish_without_receivers_is_not_a_failure() {
    let bus = CoreEventBus::new();
    assert_eq!(bus.receiver_count(), 0);

    bus.publish(CoreEvent::ConfigChanged);
    bus.publish(CoreEvent::LoggedOut {
        oj_id: "HOJ".into(),
    });

    assert_eq!(bus.receiver_count(), 0);
}

/// 发布是同步非阻塞的：不需要 await，也不等待消费者处理。
#[test]
fn publish_is_synchronous_and_does_not_block() {
    let bus = CoreEventBus::new();
    let _rx = bus.subscribe();

    let started = std::time::Instant::now();
    for _ in 0..10_000 {
        bus.publish(CoreEvent::ThemeChanged);
    }
    // 10k 次发布（消费者完全没读）应远快于 1 秒；这里只锁定「不阻塞」这一性质
    assert!(
        started.elapsed() < std::time::Duration::from_secs(1),
        "发布不应阻塞，实际耗时 {:?}",
        started.elapsed()
    );
}

/// 订阅者只收到订阅**之后**的事件（`broadcast` 不保留历史）——
/// 消费者启动时必须自行 `resync` 查询当前状态。
#[tokio::test]
async fn subscriber_does_not_receive_history() {
    let bus = CoreEventBus::new();
    bus.publish(CoreEvent::ConfigChanged);

    let mut rx = bus.subscribe();
    assert!(rx.try_recv().is_err(), "订阅前的事件不应被投递");

    bus.publish(CoreEvent::ThemeChanged);
    assert_eq!(rx.recv().await.unwrap(), CoreEvent::ThemeChanged);
}

/// 消费者落后于生产速度时收到 `Lagged`，而不是静默丢事件。
#[tokio::test]
async fn slow_receiver_observes_lagged() {
    let bus = CoreEventBus::with_capacity(2);
    let mut rx = bus.subscribe();

    for _ in 0..5 {
        bus.publish(CoreEvent::ThemeChanged);
    }

    match rx.recv().await {
        Err(tokio::sync::broadcast::error::RecvError::Lagged(lost)) => {
            assert!(lost >= 1, "应报告丢失数量");
        }
        other => panic!("期望 Lagged，实际 {other:?}"),
    }

    // Lagged 之后仍能继续消费（剩余缓存中的事件）
    assert_eq!(rx.recv().await.unwrap(), CoreEvent::ThemeChanged);
}

/// 总线析构 → 所有 receiver 收到 `Closed`（消费者据此正常退出）。
#[tokio::test]
async fn dropping_bus_closes_receivers() {
    let bus = CoreEventBus::new();
    let mut rx = bus.subscribe();
    drop(bus);

    assert!(matches!(
        rx.recv().await,
        Err(tokio::sync::broadcast::error::RecvError::Closed)
    ));
}

/// 容量 0 被钳到 1：`broadcast` 不接受 0 容量通道。
#[tokio::test]
async fn zero_capacity_is_clamped() {
    let bus = CoreEventBus::with_capacity(0);
    let mut rx = bus.subscribe();
    bus.publish(CoreEvent::ConfigChanged);
    assert_eq!(rx.recv().await.unwrap(), CoreEvent::ConfigChanged);
}

/// `receiver_count` 随订阅 / 丢弃变化（诊断信息必须真实）。
#[test]
fn receiver_count_tracks_subscribers() {
    let bus = CoreEventBus::new();
    assert_eq!(bus.receiver_count(), 0);

    let rx1 = bus.subscribe();
    let rx2 = bus.subscribe();
    assert_eq!(bus.receiver_count(), 2);

    drop(rx1);
    assert_eq!(bus.receiver_count(), 1);

    drop(rx2);
    assert_eq!(bus.receiver_count(), 0);
}

/// `Arc<CoreEventBus>` 可跨 task 共享（组合根把总线注入各消费者）。
#[tokio::test]
async fn bus_is_shareable_across_tasks() {
    let bus = Arc::new(CoreEventBus::new());
    let mut rx = bus.subscribe();

    let publisher = {
        let bus = Arc::clone(&bus);
        tokio::spawn(async move { bus.publish(CoreEvent::ThemeChanged) })
    };
    publisher.await.unwrap();

    assert_eq!(rx.recv().await.unwrap(), CoreEvent::ThemeChanged);
}
