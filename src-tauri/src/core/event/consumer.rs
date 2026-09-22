//! 受监督的事件消费者循环 —— `broadcast` 异常语义的**唯一**处理点。
//!
//! 所有异步消费者（Tauri 前端桥、审计、PluginHost、以及将来任何新增消费者）
//! 都必须经 [`spawn_consumer`] 启动，避免把 `Lagged` / `Closed` / panic 三件事
//! 在每个消费者里各写一遍（写漏一处就是「静默不消费」或「静默卡死」）。
//!
//! # 三类异常的处理约定
//!
//! | 情形 | 处理 |
//! |---|---|
//! | `Ok(event)` | 交给 `handle` |
//! | `Err(Lagged(n))` | 记录丢失数量 → 丢弃不可恢复的旧通知 → 调 `resync()` 重新查询当前状态 → 继续消费 |
//! | `Err(Closed)` | 记录日志 → 正常退出消费者 task（不是错误） |
//! | 消费者 panic | 记录日志 → 退避后**重启**（重建 receiver + 重新 `resync`）；存活超过健康阈值的重启不累计，连续超过上限则放弃该消费者 |
//!
//! # 为什么消费者失败不能影响核心业务
//!
//! 核心动作（登录 / 登出 / 保存 / 提交 / OJ 切换 / 配置落盘）由 Service 显式完成，
//! 与事件消费**没有任何等待关系**：消费者 panic 只终结它自己的 task，
//! `broadcast::Sender::send` 依旧成功（或退化为「无消费者」），业务返回值不变。
//!
//! # 为什么持有 `Weak<CoreEventBus>` 而不是 `Arc`
//!
//! 消费者若持有 `Arc<CoreEventBus>`，就等于替总线续命：总线永不析构 →
//! `RecvError::Closed` 永远不可能发生 → 「通道关闭时正常退出」这条路径成为死代码，
//! 进程退出时消费者只能被运行时强杀。持有 `Weak` 后，组合根丢弃总线即可让所有
//! 消费者收到 `Closed` 并干净退出（`ConsumerExit::Closed`）。

use std::future::Future;
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tracing::{debug, error, info, warn};

use crate::core::event::core_event::CoreEvent;
use crate::core::event::core_event_bus::CoreEventBus;

/// 单个消费者连续异常退出的最大重启次数，超过则放弃（记录 `error` 后退出）。
///
/// 「连续」的判据是 [`HEALTHY_RUN_THRESHOLD`]：worker 存活超过该阈值后的异常
/// 不累计（计数归零）—— 罕见但反复触发的 panic（如某种特殊载荷每小时命中一次）
/// 不应累计成「永久放弃」，只有真正的崩溃循环才应被上限拦住。
pub const MAX_CONSUMER_RESTARTS: u32 = 5;

/// 重启退避基数（毫秒）：第 n 次重启等待 `n * BASE`，避免异常消费者变成忙循环。
const RESTART_BACKOFF_BASE_MS: u64 = 200;

/// worker 存活达到该时长即视为「健康运行过」：之后的异常退出不累计到
/// 连续重启计数上（计数归零）。
const HEALTHY_RUN_THRESHOLD: Duration = Duration::from_secs(30);

/// 消费者需要重新同步的原因。
///
/// 显式区分「启动」与「落后」而不是只给一个 `lost` 计数：插件协议要把这件事
/// 转成一条 `ResyncRequired` 通知，插件需要知道「我刚上线」还是「我漏了事件」——
/// 后者才需要提示用户可能错过了内容。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResyncReason {
    /// 消费者刚（重）启动：receiver 收不到订阅前的事件，需补一次查询
    Startup,
    /// 消费者落后，已丢弃 `lost` 条无法恢复的通知
    Lagged(u64),
}

/// 消费者退出原因（供测试断言与诊断）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsumerExit {
    /// 事件总线已析构（通道关闭）：正常停机路径
    Closed,
    /// 连续异常退出次数超过 [`MAX_CONSUMER_RESTARTS`]
    Exhausted,
}

/// 启动一个受监督的事件消费者。
///
/// - `bus`：以 `&Arc` 传入并立即降级为 `Weak` —— 消费者**不替总线续命**，
///   组合根丢弃总线即让消费者收到 `Closed` 并退出。
/// - `name`：稳定的消费者名（日志字段，排障时用来定位是哪一个消费者出问题）。
/// - `handle`：事件处理闭包。**必须是 `Fn` 且返回 `'static` 的 future** ——
///   内部会把它放进独立 task 以实现 panic 隔离，因此需要在闭包内 `clone` 出
///   所需 `Arc`（不能借用外部栈上变量）。
/// - `resync`：重新同步回调。**消费者启动时（`Startup`）与每次 `Lagged` 之后
///   都会被调用** —— receiver 只收到订阅之后的事件，启动时补一次查询才不会漏掉
///   启动窗口；落后后补一次查询才能回到当前真值。回调本身是同步 `Fn`，
///   需要异步查询时在回调内 `tokio::spawn`。
///
/// 返回的 `JoinHandle` 产出 [`ConsumerExit`]，便于测试等待与断言。
pub fn spawn_consumer<H, Fut, R>(
    bus: &Arc<CoreEventBus>,
    name: &'static str,
    handle: H,
    resync: R,
) -> tokio::task::JoinHandle<ConsumerExit>
where
    H: Fn(CoreEvent) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
    R: Fn(ResyncReason) + Send + Sync + 'static,
{
    spawn_consumer_with_grace(bus, name, handle, resync, HEALTHY_RUN_THRESHOLD)
}

/// 带可注入「健康运行阈值」的 [`spawn_consumer`]（生产走默认阈值，测试注入小阈值）。
fn spawn_consumer_with_grace<H, Fut, R>(
    bus: &Arc<CoreEventBus>,
    name: &'static str,
    handle: H,
    resync: R,
    healthy_run_threshold: Duration,
) -> tokio::task::JoinHandle<ConsumerExit>
where
    H: Fn(CoreEvent) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
    R: Fn(ResyncReason) + Send + Sync + 'static,
{
    // **同步订阅首个 receiver**：保证「本函数返回时 receiver 已存在」。
    // 否则调用方紧接着发布的事件会在消费者首次被轮询之前丢失 ——
    // current_thread 运行时下这是必然的，生产环境里组合根启动后也可能立刻有事件。
    let first_rx = bus.subscribe();
    let bus: Weak<CoreEventBus> = Arc::downgrade(bus);
    let handle = Arc::new(handle);
    let resync = Arc::new(resync);

    tokio::spawn(async move {
        let mut restarts = 0u32;
        let mut pending_rx = Some(first_rx);
        loop {
            // 首次用同步订阅好的 receiver；重启后重建（订阅位置无法恢复，
            // 因此重启同样要靠 resync 查回当前状态）。
            let rx = match pending_rx.take() {
                Some(rx) => rx,
                None => {
                    let Some(bus) = bus.upgrade() else {
                        info!(consumer = name, "事件总线已析构，消费者退出");
                        return ConsumerExit::Closed;
                    };
                    let rx = bus.subscribe();
                    drop(bus);
                    rx
                }
            };
            resync(ResyncReason::Startup);
            info!(consumer = name, restarts, "事件消费者已启动");

            let worker_started_at = Instant::now();
            let worker = {
                let handle = Arc::clone(&handle);
                let resync = Arc::clone(&resync);
                // 独立 task = panic 隔离边界：单个事件处理 panic 只终结 worker，
                // 监督循环仍活着，可以重启消费者。
                tokio::spawn(async move { run_worker(name, rx, handle, resync).await })
            };

            match worker.await {
                Ok(()) => {
                    info!(consumer = name, "事件通道已关闭，消费者正常退出");
                    return ConsumerExit::Closed;
                }
                Err(join_error) => {
                    // 存活超过健康阈值的 worker 不算「连续故障」：罕见但反复触发
                    // 的 panic 不应累计成永久放弃，只有崩溃循环才应被上限拦住。
                    if worker_started_at.elapsed() >= healthy_run_threshold {
                        debug!(
                            consumer = name,
                            uptime_secs = worker_started_at.elapsed().as_secs(),
                            "worker 健康运行后异常退出，重启计数归零"
                        );
                        restarts = 0;
                    }
                    restarts += 1;
                    if restarts > MAX_CONSUMER_RESTARTS {
                        error!(
                            consumer = name,
                            restarts,
                            error = %join_error,
                            "事件消费者反复异常退出，放弃重启"
                        );
                        return ConsumerExit::Exhausted;
                    }
                    let backoff =
                        Duration::from_millis(RESTART_BACKOFF_BASE_MS * u64::from(restarts));
                    warn!(
                        consumer = name,
                        restarts,
                        backoff_ms = backoff.as_millis(),
                        error = %join_error,
                        "事件消费者异常退出，退避后重启"
                    );
                    tokio::time::sleep(backoff).await;
                }
            }
        }
    })
}

/// 消费者内层循环：`Ok` / `Lagged` / `Closed` 三态处理。
async fn run_worker<H, Fut, R>(
    name: &'static str,
    mut rx: broadcast::Receiver<CoreEvent>,
    handle: Arc<H>,
    resync: Arc<R>,
) where
    H: Fn(CoreEvent) -> Fut,
    Fut: Future<Output = ()>,
    R: Fn(ResyncReason),
{
    loop {
        match rx.recv().await {
            Ok(event) => {
                debug!(consumer = name, event = event.kind(), "消费者处理事件");
                handle(event).await;
            }
            Err(RecvError::Lagged(lost)) => {
                // 不能假设 broadcast 会自动补发：丢弃无法恢复的旧通知，
                // 重新查询当前状态后继续消费。
                warn!(
                    consumer = name,
                    lost, "事件消费者落后，丢弃不可恢复的通知并重新同步"
                );
                resync(ResyncReason::Lagged(lost));
            }
            Err(RecvError::Closed) => return,
        }
    }
}

#[cfg(test)]
#[path = "tests/consumer_tests.rs"]
mod tests;
