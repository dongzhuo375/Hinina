use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::core::event::app_event::{AppEvent, SystemEvent};
use crate::core::event::event_category::EventCategory;

#[test]
fn subscribe_and_publish_to_category() {
    let bus = EventBus::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let c = Arc::clone(&counter);

    bus.subscribe(EventCategory::System, Arc::new(move |_event| {
        c.fetch_add(1, Ordering::SeqCst);
    }));

    bus.publish(&AppEvent::System(SystemEvent::ConfigReloaded));
    bus.publish(&AppEvent::System(SystemEvent::ThemeChanged));

    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

#[test]
fn all_subscriber_receives_every_event() {
    let bus = EventBus::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let c = Arc::clone(&counter);

    bus.subscribe(EventCategory::All, Arc::new(move |_event| {
        c.fetch_add(1, Ordering::SeqCst);
    }));

    bus.publish(&AppEvent::System(SystemEvent::ConfigReloaded));
    bus.publish(&AppEvent::Auth(crate::core::event::app_event::AuthEvent::Logout));

    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

#[test]
fn unsubscribe_stops_receiving() {
    let bus = EventBus::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let c = Arc::clone(&counter);

    let id = bus.subscribe(EventCategory::System, Arc::new(move |_event| {
        c.fetch_add(1, Ordering::SeqCst);
    }));

    bus.publish(&AppEvent::System(SystemEvent::ConfigReloaded));
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    bus.unsubscribe(id);
    bus.publish(&AppEvent::System(SystemEvent::ThemeChanged));
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn publish_with_no_subscribers_does_not_panic() {
    let bus = EventBus::new();
    // 无订阅者时发布不应 panic
    bus.publish(&AppEvent::System(SystemEvent::ConfigReloaded));
}

#[test]
fn multiple_subscribers_same_category() {
    let bus = EventBus::new();
    let c1 = Arc::new(AtomicUsize::new(0));
    let c2 = Arc::new(AtomicUsize::new(0));
    let cc1 = Arc::clone(&c1);
    let cc2 = Arc::clone(&c2);

    bus.subscribe(EventCategory::System, Arc::new(move |_| { cc1.fetch_add(1, Ordering::SeqCst); }));
    bus.subscribe(EventCategory::System, Arc::new(move |_| { cc2.fetch_add(1, Ordering::SeqCst); }));

    bus.publish(&AppEvent::System(SystemEvent::ConfigReloaded));

    assert_eq!(c1.load(Ordering::SeqCst), 1);
    assert_eq!(c2.load(Ordering::SeqCst), 1);
}

#[test]
fn subscribe_returns_unique_ids() {
    let bus = EventBus::new();
    let dummy: EventHandler = Arc::new(|_: &AppEvent| {});

    let id1 = bus.subscribe(EventCategory::System, Arc::clone(&dummy));
    let id2 = bus.subscribe(EventCategory::System, Arc::clone(&dummy));

    assert_ne!(id1, id2);
}

#[test]
fn empty_category_cleaned_after_unsubscribe() {
    let bus = EventBus::new();
    let handler: EventHandler = Arc::new(|_| {});

    let id = bus.subscribe(EventCategory::System, handler);
    bus.unsubscribe(id);

    // 发布事件应正常（无 panic），类别已清理
    bus.publish(&AppEvent::System(SystemEvent::ConfigReloaded));
}

#[test]
fn handler_receives_correct_event_data() {
    let bus = EventBus::new();
    let received = Arc::new(std::sync::Mutex::new(String::new()));
    let r = Arc::clone(&received);

    bus.subscribe(EventCategory::System, Arc::new(move |event| {
        if let AppEvent::System(SystemEvent::ThemeChanged) = event {
            *r.lock().unwrap() = "theme-changed".to_string();
        }
    }));

    bus.publish(&AppEvent::System(SystemEvent::ThemeChanged));

    assert_eq!(*received.lock().unwrap(), "theme-changed");
}

#[test]
fn handler_panic_does_not_interrupt_other_handlers() {
    let bus = EventBus::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let c = Arc::clone(&counter);

    // 第一个 handler 会 panic
    bus.subscribe(EventCategory::System, Arc::new(|_| {
        panic!("intentional panic in handler");
    }));
    // 第二个 handler 应正常执行
    bus.subscribe(EventCategory::System, Arc::new(move |_| {
        c.fetch_add(1, Ordering::SeqCst);
    }));

    bus.publish(&AppEvent::System(SystemEvent::ConfigReloaded));

    // panic handler 不应阻止第二个 handler 执行
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

// ── 延迟投递（subscribe_deferred）──
//
// 目的：发布方不为订阅者的 I/O 买单。以下用例锁定三条契约：
// ① 发布方立即返回（不等订阅者）② 事件最终送达 ③ 隔离与过滤语义与同步投递一致。

use std::sync::mpsc;
use std::time::{Duration, Instant};

#[test]
fn deferred_subscriber_receives_event() {
    let bus = EventBus::new();
    let (tx, rx) = mpsc::channel::<String>();

    bus.subscribe_deferred(
        EventCategory::System,
        Arc::new(move |event| {
            if let AppEvent::System(SystemEvent::ThemeChanged) = event {
                let _ = tx.send("theme".to_string());
            }
        }),
    );

    bus.publish(&AppEvent::System(SystemEvent::ThemeChanged));

    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5)).expect("延迟订阅者应收到事件"),
        "theme"
    );
}

#[test]
fn deferred_subscriber_does_not_block_publisher() {
    let bus = EventBus::new();
    let (tx_done, rx_done) = mpsc::channel::<()>();

    // 订阅者阻塞 1 秒（模拟慢 I/O）：同步投递会让 publish 花掉这 1 秒
    bus.subscribe_deferred(
        EventCategory::System,
        Arc::new(move |_| {
            std::thread::sleep(Duration::from_millis(1_000));
            let _ = tx_done.send(());
        }),
    );

    let started = Instant::now();
    bus.publish(&AppEvent::System(SystemEvent::ConfigReloaded));
    let publish_elapsed = started.elapsed();

    assert!(
        publish_elapsed < Duration::from_millis(300),
        "publish 不应等待订阅者的 I/O，实际耗时 {:?}",
        publish_elapsed
    );

    // 订阅者仍在后台执行完毕
    rx_done
        .recv_timeout(Duration::from_secs(5))
        .expect("延迟订阅者最终应执行完成");
}

#[test]
fn sync_subscriber_runs_before_publish_returns_and_deferred_after() {
    let bus = EventBus::new();
    let (tx, rx) = mpsc::channel::<&'static str>();

    // 同步订阅者：publish 返回时必定已执行
    bus.subscribe(
        EventCategory::System,
        Arc::new({
            let tx = tx.clone();
            move |_| {
                let _ = tx.send("sync");
            }
        }),
    );
    // 延迟订阅者：可能晚于 publish 返回
    bus.subscribe_deferred(
        EventCategory::System,
        Arc::new(move |_| {
            let _ = tx.send("deferred");
        }),
    );

    bus.publish(&AppEvent::System(SystemEvent::ConfigReloaded));

    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5)).expect("同步订阅者应先执行"),
        "sync"
    );
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5)).expect("延迟订阅者随后执行"),
        "deferred"
    );
}

#[test]
fn deferred_subscriber_respects_category_filter() {
    let bus = EventBus::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let c = Arc::clone(&counter);

    bus.subscribe_deferred(
        EventCategory::System,
        Arc::new(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        }),
    );

    // 非 System 类别：不应触发
    bus.publish(&AppEvent::Auth(crate::core::event::app_event::AuthEvent::Logout));
    // 用一个 System 事件确认订阅确实生效（同时充当「前面的 publish 未触发」的时序屏障）
    bus.publish(&AppEvent::System(SystemEvent::ConfigReloaded));

    let deadline = Instant::now() + Duration::from_secs(5);
    while counter.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "延迟订阅者只应对其订阅类别的事件触发"
    );
}

#[test]
fn deferred_panic_does_not_kill_worker() {
    let bus = EventBus::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let c = Arc::clone(&counter);

    // 先注册一个必 panic 的延迟订阅者
    bus.subscribe_deferred(
        EventCategory::System,
        Arc::new(|_| panic!("intentional panic in deferred handler")),
    );
    bus.subscribe_deferred(
        EventCategory::System,
        Arc::new(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        }),
    );

    bus.publish(&AppEvent::System(SystemEvent::ConfigReloaded));
    bus.publish(&AppEvent::System(SystemEvent::ThemeChanged));

    let deadline = Instant::now() + Duration::from_secs(5);
    while counter.load(Ordering::SeqCst) < 2 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        counter.load(Ordering::SeqCst),
        2,
        "延迟订阅者 panic 不应终止消费线程（后续事件仍应送达）"
    );
}

#[test]
fn unsubscribe_stops_deferred_delivery() {
    let bus = EventBus::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let c = Arc::clone(&counter);

    let id = bus.subscribe_deferred(
        EventCategory::System,
        Arc::new(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        }),
    );
    bus.unsubscribe(id);

    bus.publish(&AppEvent::System(SystemEvent::ConfigReloaded));
    std::thread::sleep(Duration::from_millis(200));

    assert_eq!(
        counter.load(Ordering::SeqCst),
        0,
        "取消订阅后不应再收到延迟投递"
    );
}
