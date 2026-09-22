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

/// 存活超过健康阈值的 worker 异常退出**不累计**重启计数：罕见但反复触发的
/// panic（如某种特殊载荷）不会把消费者拖入 `Exhausted` —— 只有崩溃循环才会。
#[tokio::test]
async fn healthy_run_resets_restart_counter() {
    let bus = Arc::new(CoreEventBus::new());
    let reasons = Arc::new(Mutex::new(Vec::new()));

    let task = spawn_consumer_with_grace(
        &bus,
        "test-healthy-reset",
        |_event: CoreEvent| async move { panic!("always panic") },
        recording_resync(&reasons),
        // 阈值压到 50ms：worker 空闲存活 80ms 后再收到事件即视为「健康运行过」
        Duration::from_millis(50),
    );

    let resync_count = || reasons.lock().unwrap_or_else(|e| e.into_inner()).len();
    // 每轮：发布 → panic → 重启（resync +1）→ 空闲存活超过阈值 → 下一轮
    for _ in 0..(MAX_CONSUMER_RESTARTS + 2) {
        bus.publish(CoreEvent::ThemeChanged);
        let before = resync_count();
        wait_until(
            || resync_count() > before || task.is_finished(),
            "panic 后应触发重启（或放弃）",
        )
        .await;
        assert!(
            !task.is_finished(),
            "健康存活后的重启不应累计到放弃阈值"
        );
        tokio::time::sleep(Duration::from_millis(80)).await;
    }

    // 全程重启了 MAX+2 次仍未 Exhausted：计数确实被健康存活归零了
    assert!(resync_count() >= usize::try_from(MAX_CONSUMER_RESTARTS + 2).unwrap());
}

/// 真正的崩溃循环（worker 存活不超过健康阈值）仍会被上限拦住：
/// 连续异常退出超过 [`MAX_CONSUMER_RESTARTS`] 次后放弃该消费者。
#[tokio::test]
async fn rapid_crash_loop_exhausts_consumer() {
    let bus = Arc::new(CoreEventBus::new());
    let reasons = Arc::new(Mutex::new(Vec::new()));

    let task = spawn_consumer_with_grace(
        &bus,
        "test-exhaust",
        |_event: CoreEvent| async move { panic!("always panic") },
        recording_resync(&reasons),
        // 阈值放大到 1 小时：任何快速崩溃都不算「健康运行过」
        Duration::from_secs(3600),
    );

    let resync_count = || reasons.lock().unwrap_or_else(|e| e.into_inner()).len();
    // 逐条发布并等每次重启完成（重启后的 receiver 收不到订阅前的事件，
    // 不能一次性发布排队）。resync 总数 = 初始 Startup 1 次 + 每次重启 1 次，
    // 用绝对目标计数，避免「发布后读基线」与 Startup resync 竞争错位。
    for n in 1..=MAX_CONSUMER_RESTARTS {
        bus.publish(CoreEvent::ThemeChanged);
        let target = usize::try_from(n + 1).expect("重启计数不会溢出 usize");
        wait_until(
            || resync_count() >= target,
            "panic 后应触发重启",
        )
        .await;
    }

    // 第 MAX+1 次异常退出：不再重启，消费者被放弃
    bus.publish(CoreEvent::ThemeChanged);
    let exit = tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .expect("崩溃循环应在上限内被放弃")
        .expect("消费者 task 不应 panic");
    assert_eq!(exit, ConsumerExit::Exhausted);
}
