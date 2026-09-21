use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::*;

/// 轮询等待条件成立（上限 5 秒），失败即断言失败。
///
/// 用轮询而非固定 `sleep`：消费者启动 / 重启的时机由调度器决定，
/// 固定睡眠会让用例在慢机器上偶发失败。
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

/// 计数器消费者：记录收到的事件数。
fn counting_consumer(
    counter: &Arc<AtomicUsize>,
) -> impl Fn(CoreEvent) -> std::future::Ready<()> + Send + Sync + 'static {
    let counter = Arc::clone(counter);
    move |_event: CoreEvent| {
        counter.fetch_add(1, Ordering::SeqCst);
        std::future::ready(())
    }
}

/// 记录 resync 原因的回调。
fn recording_resync(
    reasons: &Arc<Mutex<Vec<ResyncReason>>>,
) -> impl Fn(ResyncReason) + Send + Sync + 'static {
    let reasons = Arc::clone(reasons);
    move |reason: ResyncReason| {
        reasons
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(reason);
    }
}

/// 无操作 resync（不需要断言重新同步的用例用）。
fn noop_resync(_reason: ResyncReason) {}

#[tokio::test]
async fn consumer_receives_published_events() {
    let bus = Arc::new(CoreEventBus::new());
    let handled = Arc::new(AtomicUsize::new(0));

    let _task = spawn_consumer(&bus, "test-basic", counting_consumer(&handled), noop_resync);

    bus.publish(CoreEvent::ConfigChanged);
    bus.publish(CoreEvent::ThemeChanged);

    wait_until(
        || handled.load(Ordering::SeqCst) >= 2,
        "消费者应收到两条事件",
    )
    .await;
}

/// 多个消费者互不干扰：各自都收到全部事件（`broadcast` 扇出）。
#[tokio::test]
async fn consumers_are_independent() {
    let bus = Arc::new(CoreEventBus::new());
    let a = Arc::new(AtomicUsize::new(0));
    let b = Arc::new(AtomicUsize::new(0));

    let _ta = spawn_consumer(&bus, "test-a", counting_consumer(&a), noop_resync);
    let _tb = spawn_consumer(&bus, "test-b", counting_consumer(&b), noop_resync);

    for _ in 0..3 {
        bus.publish(CoreEvent::ThemeChanged);
    }

    wait_until(|| a.load(Ordering::SeqCst) >= 3, "消费者 A 收到 3 条").await;
    wait_until(|| b.load(Ordering::SeqCst) >= 3, "消费者 B 收到 3 条").await;
}

/// 消费者启动时必须先 `resync(Startup)`：receiver 收不到订阅前的事件，
/// 不补一次查询就会漏掉启动窗口内的状态变化。
#[tokio::test]
async fn resync_runs_on_startup() {
    let bus = Arc::new(CoreEventBus::new());
    let reasons = Arc::new(Mutex::new(Vec::new()));

    let _task = spawn_consumer(
        &bus,
        "test-resync",
        |_event: CoreEvent| std::future::ready(()),
        recording_resync(&reasons),
    );

    wait_until(
        || !reasons.lock().unwrap_or_else(|e| e.into_inner()).is_empty(),
        "启动时应调用一次 resync",
    )
    .await;
    assert_eq!(
        reasons.lock().unwrap_or_else(|e| e.into_inner())[0],
        ResyncReason::Startup
    );
}

/// 消费者落后（`Lagged`）→ 以 `Lagged(n)` 触发重新同步并继续消费，不静默停摆。
#[tokio::test]
async fn lagged_triggers_resync_and_keeps_consuming() {
    // 容量 1 + 慢消费者：必然落后
    let bus = Arc::new(CoreEventBus::with_capacity(1));
    let reasons = Arc::new(Mutex::new(Vec::new()));
    let handled = Arc::new(AtomicUsize::new(0));

    let h = Arc::clone(&handled);
    let _task = spawn_consumer(
        &bus,
        "test-lagged",
        move |_event: CoreEvent| {
            let h = Arc::clone(&h);
            async move {
                tokio::time::sleep(Duration::from_millis(30)).await;
                h.fetch_add(1, Ordering::SeqCst);
            }
        },
        recording_resync(&reasons),
    );

    for _ in 0..20 {
        bus.publish(CoreEvent::ThemeChanged);
    }

    wait_until(
        || {
            reasons
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .iter()
                .any(|reason| matches!(reason, ResyncReason::Lagged(_)))
        },
        "落后后应触发 Lagged 重新同步",
    )
    .await;
    // 落后之后仍在消费
    wait_until(|| handled.load(Ordering::SeqCst) >= 1, "落后后应继续消费").await;
}

/// 总线析构 → 消费者收到 `Closed` 并干净退出（不是被强杀）。
#[tokio::test]
async fn dropping_bus_exits_consumer_cleanly() {
    let bus = Arc::new(CoreEventBus::new());
    let task = spawn_consumer(
        &bus,
        "test-closed",
        |_event: CoreEvent| std::future::ready(()),
        noop_resync,
    );

    drop(bus);

    let exit = task.await.expect("消费者 task 不应 panic");
    assert_eq!(exit, ConsumerExit::Closed);
}

/// 单个事件处理 panic → 消费者被重启，后续事件仍能送达。
#[tokio::test]
async fn panic_in_handler_restarts_consumer() {
    let bus = Arc::new(CoreEventBus::new());
    let attempts = Arc::new(AtomicUsize::new(0));
    let handled = Arc::new(AtomicUsize::new(0));
    let reasons = Arc::new(Mutex::new(Vec::new()));

    let a = Arc::clone(&attempts);
    let h = Arc::clone(&handled);
    let _task = spawn_consumer(
        &bus,
        "test-panic",
        move |_event: CoreEvent| {
            let a = Arc::clone(&a);
            let h = Arc::clone(&h);
            async move {
                if a.fetch_add(1, Ordering::SeqCst) == 0 {
                    panic!("intentional panic in consumer handler");
                }
                h.fetch_add(1, Ordering::SeqCst);
            }
        },
        recording_resync(&reasons),
    );

    bus.publish(CoreEvent::ConfigChanged);
    // 首次处理 panic；重启会再次 resync（启动 1 次 + 重启 1 次）
    wait_until(
        || {
            attempts.load(Ordering::SeqCst) >= 1
                && reasons.lock().unwrap_or_else(|e| e.into_inner()).len() >= 2
        },
        "panic 后消费者应被重启",
    )
    .await;

    // 重启后发布的事件仍被处理
    bus.publish(CoreEvent::ThemeChanged);
    wait_until(
        || handled.load(Ordering::SeqCst) >= 1,
        "重启后应继续处理事件",
    )
    .await;
}

/// 一个消费者反复 panic **不影响**另一个消费者，也不影响发布方 ——
/// 核心业务动作与事件消费之间没有任何等待关系。
#[tokio::test]
async fn failing_consumer_does_not_affect_others() {
    let bus = Arc::new(CoreEventBus::new());
    let healthy = Arc::new(AtomicUsize::new(0));

    let _bad = spawn_consumer(
        &bus,
        "test-bad",
        |_event: CoreEvent| async move { panic!("always panic") },
        noop_resync,
    );
    let _good = spawn_consumer(&bus, "test-good", counting_consumer(&healthy), noop_resync);

    // 发布方（核心业务动作）照常返回，不因消费者 panic 失败
    for _ in 0..3 {
        bus.publish(CoreEvent::ConfigChanged);
    }

    wait_until(
        || healthy.load(Ordering::SeqCst) >= 3,
        "健康消费者不受失败消费者影响",
    )
    .await;
}
