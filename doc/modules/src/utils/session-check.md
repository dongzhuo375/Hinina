# session-check（赛前会话预检调度策略）

> 源文件：`src/utils/session-check.ts`

## 职责

以纯函数定义"何时校验会话"的调度策略，把全场客户端的预检请求散布在时间窗口内，避免开赛前对 OJ 服务器形成同步尖峰。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `PrecheckReason` | `'periodic' \| 'window' \| 'immediate'` | 预检类型，决定本次校验后是否继续排程（仅 `periodic` 继续） |
| `PrecheckPlan` | `{ delayMs: number; reason: PrecheckReason }` | 一次排程结果 |
| `planNextPrecheck` | `(nowMs, startMs, rand?) => PrecheckPlan \| null` | 计算下一次预检延迟；`null` 表示不应再预检 |
| `planRetryDelayMs` | `(rand?) => number` | `unknown` 后的单次重试延迟（2–5s 随机） |

常量：`PRECHECK_WINDOW_START_MS`(10min)、`PRECHECK_WINDOW_END_MS`(3min)、`PRECHECK_STOP_MS`(30s)、`PERIODIC_RECHECK_MS`(5min)、`PERIODIC_JITTER_MS`(±60s)、`IMMEDIATE_JITTER_MS`(3s)、`RETRY_BACKOFF_MIN/MAX_MS`(2s/5s)。

## 直接依赖

无（纯函数，随机源通过参数注入）

## 被依赖

- `views/LoginView.vue` — 等待开赛期间的预检排程与重试

## 逻辑流程

```
untilStart = startMs - nowMs

untilStart ≤ 30s                    → null       不再预检，交给进场流程与全局 401 兜底
untilStart > 10min                  → periodic   min(5min ± 60s, 到窗口左边界的时间)
3min < untilStart ≤ 10min           → window     rand() × 剩余窗口长度（一次性，全场错峰）
untilStart ≤ 3min（迟到启动等）      → immediate  rand() × 3s 抖动后立即执行
```

设计要点：

- **进场不错峰**：T-0 导航是公平性要求，且此时服务端峰值来自赛制本身，不应削平；
  错峰的只是"校验会话"这类可提前、可分散的请求。
- **窗口内一次性**：`window` / `immediate` 预检执行后不再排程，避免在窗口内反复校验；
  只有 `periodic` 会继续，直到进入窗口后自然收敛为一次。
- **只重试一次**：`unknown` 通常是机房链路问题，重试风暴只会加重拥塞。
- 随机源可注入（`rand` 参数），便于将来用 Vitest 做确定性单元测试。
