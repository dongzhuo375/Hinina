# consumer

## 职责
受监督的事件消费者循环 —— `broadcast` 异常语义的**唯一**处理点。所有异步消费者（Tauri 前端桥、审计、PluginHost、以及将来任何新增消费者）都必须经 `spawn_consumer` 启动，避免把 `Lagged` / `Closed` / panic 三件事在每个消费者里各写一遍（写漏一处就是「静默不消费」或「静默卡死」）。

## 核心类型/函数
- **`MAX_CONSUMER_RESTARTS`**（5）— 单个消费者**连续**异常退出的最大重启次数，超过则记录 `error` 后放弃（`ConsumerExit::Exhausted`）。
  「连续」的判据是 `HEALTHY_RUN_THRESHOLD`（**30s**，私有常量）：worker 存活超过该阈值后的异常**不累计**（重启计数归零）—— 罕见但反复触发的 panic（如某种特殊载荷每小时命中一次）不应累计成「永久放弃」，只有真正的崩溃循环才应被上限拦住。代价是**周期 ≥30s 的慢速崩溃循环永不 `Exhausted`**（有意取舍）。
- **`ResyncReason`** — `Startup` / `Lagged(u64)`。显式区分而不是只给一个计数：插件协议要把这件事转成 `ResyncRequired`，插件需要知道「我刚上线」还是「我漏了事件」。
- **`ConsumerExit`** — `Closed`（总线析构，正常停机）/ `Exhausted`（重启次数超限）。作为 `JoinHandle` 的产出，供组合根观察与测试断言。
- **`spawn_consumer(bus: &Arc<CoreEventBus>, name, handle, resync) -> JoinHandle<ConsumerExit>`**
  - `bus` 以 `&Arc` 传入并**立即降级为 `Weak`**：消费者不替总线续命，否则 `Closed` 永远不可能发生、该分支成为死代码。
  - `handle: Fn(CoreEvent) -> Fut + Send + Sync + 'static`：内部放进独立 task（panic 隔离边界），因此需在闭包内 `clone` 出所需 `Arc`。
  - `resync: Fn(ResyncReason) + Send + Sync + 'static`：**消费者启动时与每次 `Lagged` 之后都会调用**。同步 `Fn`，需要异步查询时在回调内 `tokio::spawn`。
- **`run_worker`**（内部）— `Ok` / `Lagged` / `Closed` 三态循环。

## 直接依赖
- `tokio::sync::broadcast`（`Receiver`、`RecvError`）
- `tokio::task` / `tokio::time`（spawn、退避 sleep）
- `std::sync::{Arc, Weak}`
- `tracing::{debug, error, info, warn}`
- `core::event::core_event::CoreEvent`、`core::event::core_event_bus::CoreEventBus`

## 被依赖
- `main.rs`（前端事件桥、启动消费者）
- `infra::audit`（审计消费者）
- `plugin::host::plugin_host::PluginHost`（插件事件消费者）

## 逻辑流程
监督循环（外层）→ 每次（重）启：`bus.upgrade()`（失败 = 总线已析构 → 返回 `Closed`）→ `subscribe()` → `resync(Startup)` → 记 `worker_started_at` → `tokio::spawn(run_worker)` → 等待 worker：
- `Ok(())` → 通道关闭 → 记日志 → 返回 `Closed`；
- `Err(JoinError)`（panic）→ **先看 `worker_started_at.elapsed() >= HEALTHY_RUN_THRESHOLD`：达到则 `restarts = 0`（健康运行过）**，再 `restarts += 1` → 超过上限则返回 `Exhausted`，否则退避 `n * 200ms` 后重启。

worker（内层）：`recv().await` → `Ok` 交给 `handle`；`Lagged(lost)` 记 `warn` 并 `resync(Lagged(lost))` 后继续；`Closed` 返回。

> 归零发生在自增**之前**，语义精确为「距该 worker 启动 <30s 就崩」才算连续故障。

## 关键不变量
- **消费者失败不影响核心业务**：核心动作由 Service 显式完成，与事件消费没有任何等待关系；消费者 panic 只终结它自己的 task，`publish` 依旧成功（或退化为「无消费者」），返回值不变。
- **`Lagged` 不等于自动补发**：必须重新查询当前状态。
- **重启会重建 receiver**：期间的订阅位置无法恢复，因此重启后同样靠 `resync` 查回当前状态。
- **首次 receiver 在 `spawn_consumer` 调用内同步订阅**：保证「函数返回时 receiver 已存在」，否则调用方紧接着发布的事件会在消费者首次被轮询之前丢失（current_thread 运行时下必然发生）。

## 测试
`tests/consumer_tests.rs`：收到事件、多消费者互不干扰、启动即 `resync(Startup)`、落后触发 `Lagged` 重新同步并继续消费、总线析构后干净退出（`ConsumerExit::Closed`）、handler panic 后重启并继续处理、失败消费者不影响健康消费者与发布方、**健康运行后异常不累计**（`healthy_run_resets_restart_counter`：注入短阈值 + worker 存活更久，6 轮后仍不 `Exhausted`）、**紧凑崩溃循环仍会放弃**（`rapid_crash_loop_exhausts_consumer`：阈值设为 1 小时排除归零，5 次上限后返回 `Exhausted`）。
