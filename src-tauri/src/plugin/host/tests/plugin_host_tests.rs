use super::*;
use crate::plugin::api::event::PluginEvent;
use crate::plugin::host::manifest::PluginManifest;
use std::time::Duration;

fn grant(id: &str, approved: &[PluginPermission]) -> PluginGrant {
    let manifest: PluginManifest = serde_json::from_value(serde_json::json!({
        "id":id,"name":"Demo","version":"1.0.0","manifestVersion":2,"apiVersion":2,
        "runtime":"js","entry":"index.js",
        "permissions":["events.subscribe","submission.read","contest.read","notification"],
        "events":["submission.judged","system.config_changed","system.theme_changed"]
    }))
    .unwrap();
    PluginGrant::authorize(&manifest, approved).unwrap()
}

fn full_grant(id: &str) -> PluginGrant {
    grant(
        id,
        &[
            PluginPermission::EventsSubscribe,
            PluginPermission::SubmissionRead,
            PluginPermission::ContestRead,
        ],
    )
}

fn host() -> (Arc<PluginHost>, Arc<CoreEventBus>) {
    let bus = Arc::new(CoreEventBus::new());
    (Arc::new(PluginHost::new(Arc::clone(&bus))), bus)
}

async fn receive(rx: &mut mpsc::Receiver<PluginEventEnvelope>) -> PluginEventEnvelope {
    tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("delivery timed out")
        .expect("unexpectedly closed")
}

#[test]
fn notification_or_declaration_does_not_grant_event_subscription() {
    let (host, _bus) = host();
    let denied = grant("com.example.denied", &[PluginPermission::Notification]);
    assert!(matches!(
        host.subscribe(&denied),
        Err(PluginHostError::PermissionDenied)
    ));
    assert_eq!(host.subscriber_count(), 0);
}

#[test]
fn subscription_and_payload_permissions_filter_before_sequence_assignment() {
    let (host, _bus) = host();
    let mut rx = host
        .subscribe(&grant(
            "com.example.limited",
            &[PluginPermission::EventsSubscribe],
        ))
        .unwrap();
    assert_eq!(rx.try_recv().unwrap().sequence, 1);
    host.dispatch(&CoreEvent::SubmissionJudged {
        submission_id: "s1".into(),
        status: "Accepted".into(),
    });
    host.dispatch(&CoreEvent::TokenRotated {
        oj_id: "hoj".into(),
    });
    host.dispatch(&CoreEvent::ContestSelected {
        contest_id: "c1".into(),
    });
    host.dispatch(&CoreEvent::ConfigChanged);
    let next = rx.try_recv().unwrap();
    assert_eq!(next.sequence, 2);
    assert_eq!(next.event, PluginEvent::ResyncRequired { lost: 0 });
    let config = rx.try_recv().unwrap();
    assert_eq!(config.sequence, 3);
    assert_eq!(config.event, PluginEvent::ConfigChanged);
    assert!(rx.try_recv().is_err());
}

#[test]
fn manifest_event_allowlist_is_respected_even_with_full_grants() {
    let (host, _bus) = host();
    let mut manifest = full_grant("com.example.onlyjudged").manifest().clone();
    manifest.events = vec![PluginEventSubscription::SubmissionJudged];
    let grant = PluginGrant::authorize(&manifest, &manifest.permissions).unwrap();
    let mut rx = host.subscribe(&grant).unwrap();
    rx.try_recv().unwrap();
    host.dispatch(&CoreEvent::ConfigChanged);
    host.dispatch(&CoreEvent::SubmissionJudged {
        submission_id: "s1".into(),
        status: "WrongAnswer".into(),
    });
    let envelope = rx.try_recv().unwrap();
    assert_eq!(envelope.sequence, 2);
    assert!(
        matches!(envelope.event,PluginEvent::SubmissionJudged { status,.. } if status=="WrongAnswer")
    );
}

#[test]
fn concurrent_duplicate_registration_has_one_winner_and_one_initial_notice() {
    let (host, _bus) = host();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let tasks: Vec<_> = (0..2)
        .map(|_| {
            let host = Arc::clone(&host);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                host.subscribe(&full_grant("com.example.same"))
            })
        })
        .collect();
    let mut results: Vec<_> = tasks.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Err(PluginHostError::AlreadySubscribed)))
            .count(),
        1
    );
    let mut winner = results
        .swap_remove(results.iter().position(|r| r.is_ok()).unwrap())
        .unwrap();
    assert_eq!(winner.try_recv().unwrap().sequence, 1);
    assert!(winner.try_recv().is_err());
    assert_eq!(host.subscriber_count(), 1);
}

#[test]
fn queue_full_disconnects_only_slow_plugin_and_reconnect_starts_with_resync() {
    let (host, _bus) = host();
    let mut slow = host
        .subscribe_with_capacity(&full_grant("com.example.slow"), 1)
        .unwrap();
    let mut fast = host.subscribe(&full_grant("com.example.fast")).unwrap();
    fast.try_recv().unwrap();
    host.dispatch(&CoreEvent::ConfigChanged); // slow's initial notice still fills its queue
    assert_eq!(host.subscriber_count(), 1);
    assert_eq!(fast.try_recv().unwrap().event, PluginEvent::ConfigChanged);
    assert_eq!(
        slow.try_recv().unwrap().event,
        PluginEvent::ResyncRequired { lost: 0 }
    );
    assert!(matches!(
        slow.try_recv(),
        Err(mpsc::error::TryRecvError::Disconnected)
    ));
    let mut retry = host.subscribe(&full_grant("com.example.slow")).unwrap();
    let notice = retry.try_recv().unwrap();
    assert_eq!(notice.sequence, 1);
    assert_eq!(notice.event, PluginEvent::ResyncRequired { lost: 0 });
}

#[test]
fn closed_receiver_is_removed_without_affecting_other_subscribers() {
    let (host, _bus) = host();
    drop(host.subscribe(&full_grant("com.example.closed")).unwrap());
    let mut healthy = host.subscribe(&full_grant("com.example.healthy")).unwrap();
    healthy.try_recv().unwrap();
    host.dispatch(&CoreEvent::ThemeChanged);
    assert_eq!(host.subscriber_count(), 1);
    assert_eq!(healthy.try_recv().unwrap().event, PluginEvent::ThemeChanged);
}

#[test]
fn unsubscribe_is_idempotent_and_prevents_further_enqueues() {
    let (host, _bus) = host();
    let mut rx = host.subscribe(&full_grant("com.example.demo")).unwrap();
    rx.try_recv().unwrap();
    host.dispatch(&CoreEvent::ConfigChanged);
    assert!(host.unsubscribe("com.example.demo"));
    assert!(!host.unsubscribe("com.example.demo"));
    host.dispatch(&CoreEvent::ThemeChanged);
    assert_eq!(rx.try_recv().unwrap().event, PluginEvent::ConfigChanged); // previously buffered
    assert!(matches!(
        rx.try_recv(),
        Err(mpsc::error::TryRecvError::Disconnected)
    ));
}

#[test]
fn sequences_are_per_subscription_and_resync_is_mandatory() {
    let (host, _bus) = host();
    let mut a = host.subscribe(&full_grant("com.example.a")).unwrap();
    a.try_recv().unwrap();
    host.dispatch(&CoreEvent::ConfigChanged);
    let mut b = host.subscribe(&full_grant("com.example.b")).unwrap();
    assert_eq!(
        b.try_recv().unwrap().event,
        PluginEvent::ResyncRequired { lost: 0 }
    );
    host.dispatch(&CoreEvent::ThemeChanged);
    assert_eq!(a.try_recv().unwrap().sequence, 2);
    assert_eq!(a.try_recv().unwrap().sequence, 3);
    assert_eq!(b.try_recv().unwrap().sequence, 2);
}

#[tokio::test]
async fn host_consumer_delivers_bus_events_and_exits_when_bus_is_dropped() {
    let (host, bus) = host();
    assert_eq!(bus.receiver_count(), 0);
    let mut rx = host.subscribe(&full_grant("com.example.demo")).unwrap();
    assert_eq!(receive(&mut rx).await.sequence, 1);
    let task = host.start().unwrap();
    assert_eq!(bus.receiver_count(), 1);
    assert!(matches!(host.start(), Err(PluginHostError::AlreadyStarted)));
    assert_eq!(bus.receiver_count(), 1);
    assert_eq!(
        receive(&mut rx).await.event,
        PluginEvent::ResyncRequired { lost: 0 }
    );
    bus.publish(CoreEvent::SubmissionJudged {
        submission_id: "s1".into(),
        status: "Accepted".into(),
    });
    let envelope = receive(&mut rx).await;
    assert_eq!(envelope.sequence, 3);
    assert_eq!(
        envelope.event,
        PluginEvent::SubmissionJudged {
            submission_id: "s1".into(),
            status: "Accepted".into()
        }
    );
    drop(host);
    drop(bus);
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap(),
        ConsumerExit::Closed
    );
}

#[tokio::test]
async fn late_subscription_begins_with_resync_before_bus_events() {
    let (host, bus) = host();
    let mut canary = host.subscribe(&full_grant("com.example.canary")).unwrap();
    receive(&mut canary).await;
    let task = host.start().unwrap();
    receive(&mut canary).await;
    let mut late = host.subscribe(&full_grant("com.example.late")).unwrap();
    assert_eq!(receive(&mut late).await.sequence, 1);
    bus.publish(CoreEvent::ConfigChanged);
    let next = receive(&mut late).await;
    assert_eq!(next.sequence, 2);
    assert_eq!(next.event, PluginEvent::ConfigChanged);
    drop(host);
    drop(bus);
    assert_eq!(task.await.unwrap(), ConsumerExit::Closed);
}

#[tokio::test]
async fn lagged_receiver_gets_resync_without_replay() {
    let bus = Arc::new(CoreEventBus::with_capacity(1));
    let host = Arc::new(PluginHost::new(Arc::clone(&bus)));
    let mut rx = host.subscribe(&full_grant("com.example.demo")).unwrap();
    receive(&mut rx).await;
    let task = host.start().unwrap();
    receive(&mut rx).await;
    // current_thread runtime: no await until all sends complete, deterministically lagging.
    for _ in 0..50 {
        bus.publish(CoreEvent::ThemeChanged);
    }
    let notice = receive(&mut rx).await;
    assert!(matches!(notice.event,PluginEvent::ResyncRequired {lost} if lost>0));
    assert_eq!(receive(&mut rx).await.event, PluginEvent::ThemeChanged);
    drop(host);
    drop(bus);
    assert_eq!(task.await.unwrap(), ConsumerExit::Closed);
}

#[test]
fn resync_queue_overflow_also_disconnects_only_the_full_subscription() {
    let (host, _bus) = host();
    let mut slow = host
        .subscribe_with_capacity(&full_grant("com.example.slow"), 1)
        .unwrap();
    let mut fast = host.subscribe(&full_grant("com.example.fast")).unwrap();
    fast.try_recv().unwrap();
    notify_resync(&host.subscriptions, 10);
    assert_eq!(
        fast.try_recv().unwrap().event,
        PluginEvent::ResyncRequired { lost: 10 }
    );
    slow.try_recv().unwrap();
    assert!(matches!(
        slow.try_recv(),
        Err(mpsc::error::TryRecvError::Disconnected)
    ));
    assert_eq!(host.subscriber_count(), 1);
}

#[test]
fn explicit_resync_follows_buffered_business_events_and_reports_closed_or_full_queue() {
    let (host, _bus) = host();
    let id = "com.example.resync";
    let mut rx = host.subscribe(&full_grant(id)).unwrap();
    rx.try_recv().unwrap();
    host.dispatch(&CoreEvent::ConfigChanged);
    assert!(host.request_resync(id));
    assert_eq!(rx.try_recv().unwrap().event, PluginEvent::ConfigChanged);
    let notice = rx.try_recv().unwrap();
    assert_eq!(notice.event, PluginEvent::ResyncRequired { lost: 0 });
    assert_eq!(notice.sequence, 3);
    drop(rx);
    assert!(!host.request_resync(id));
    assert_eq!(host.subscriber_count(), 0);
    let _full = host.subscribe_with_capacity(&full_grant(id), 1).unwrap();
    assert!(!host.request_resync(id));
    assert_eq!(host.subscriber_count(), 0);
}

#[test]
fn subscription_and_overflow_logs_never_hold_the_subscription_lock() {
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};
    // The lock assertion only runs when a log event actually reaches the writer, so the
    // test must also prove events were received. Callsite interest is a process-global
    // cache; without this counter the test could pass vacuously if a parallel test had
    // cached `never` for these callsites and silently dropped every event.
    let writes = Arc::new(AtomicUsize::new(0));
    #[derive(Clone)]
    struct LockCheckingWriter(std::sync::Weak<PluginHost>, Arc<AtomicUsize>);
    impl Write for LockCheckingWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if let Some(host) = self.0.upgrade() {
                assert!(
                    host.subscriptions.try_lock().is_ok(),
                    "logging inside subscription lock"
                );
            }
            self.1.fetch_add(1, Ordering::SeqCst);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let (host, _bus) = host();
    let writer = LockCheckingWriter(Arc::downgrade(&host), writes.clone());
    let subscriber = tracing_subscriber::fmt()
        .with_writer(move || writer.clone())
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        let _receiver = host
            .subscribe_with_capacity(&full_grant("test.log"), 1)
            .unwrap();
        host.dispatch(&CoreEvent::ConfigChanged);
        let _receiver = host
            .subscribe_with_capacity(&full_grant("test.log"), 1)
            .unwrap();
        assert!(!host.request_resync("test.log"));
    });
    // The overflow/resync failures above must have logged at least once.
    assert!(
        writes.load(Ordering::SeqCst) > 0,
        "no log events reached the writer; the lock assertion never ran"
    );
}
