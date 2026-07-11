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
