# polling（可复用轮询原语）

> 源文件：`src/utils/polling.ts`

## 职责

提供带抖动错峰、重入保护与可注入定时器的通用轮询器（榜单 10s±2s、题目总览 30s±5s 等场景共用），把"周期性拉数据"的节奏控制从各 store 中抽出。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `planPollDelayMs` | `(intervalMs, jitterMs, rand?) => number` | 计算一次延迟：`intervalMs ± jitterMs` 均匀分布，结果钳到非负 |
| `PollerOptions` | interface | `task` / `intervalMs` / `jitterMs?` / `isPaused?` / `onError?` / `rand?` / `setTimeoutFn?` / `clearTimeoutFn?` |
| `Poller` | interface | `start()`（幂等）/ `stop()`（幂等，清理句柄）/ `isRunning()` |
| `createPoller` | `(options: PollerOptions) => Poller` | 创建递归 setTimeout 轮询器 |

## 直接依赖

无（纯模块，随机源与定时器函数均通过参数注入，默认 `Math.random` / `setTimeout` / `clearTimeout`）

## 被依赖

- `stores/rankStore.ts` — `startLive()` 创建榜单轮询器（10s ± 2s）
- `views/ProblemSetView.vue` — 题目总览的状态轮询（切后台暂停，卸载时停止）
- `utils/__tests__/polling.spec.ts` — 单元测试（注入假定时器与随机源）

## 逻辑流程

```
start() ─ running=true ─ scheduleNext()
                            └ setTimeout(tick, planPollDelayMs(interval, jitter, rand))

tick():
  handle = null
  !running → return                 // stop() 后残留触发直接退出
  scheduleNext()                      // 先续排下一周期：暂停/重入只跳过本次 task，不打断节奏
  busy → return                       // 重入保护
  isPaused?.() → return               // 如页面隐藏
  runTask():
    busy = true
    task() 同步返回 → busy = false
    task() 返回 Promise → settle 时 busy = false；reject → busy = false + onError(e)
    task() 同步抛错 → 同上（轮询不终止）
```

设计要点：

- **抖动错峰**：全场客户端往往在同一时刻进入视图（如开赛瞬间），若间隔固定，各客户端请求相位
  永久同步，每个周期都在同一毫秒打到 OJ 服务器形成周期性尖峰。±jitter 均匀抖动让相位逐周期
  随机打散，尖峰摊平成均匀流量（HOJ 榜单文档 §9.9 也要求轮询间隔 ≥10s 且后台暂停）。
- **递归 setTimeout 而非 setInterval**：setInterval 的间隔在创建时固定，无法让抖动逐周期生效；
  每次 tick 后重新计算下一周期延迟才能持续错峰。
- **重入保护**：上一次 task（含异步）未 settle 时到点，只续排、跳过本次执行，绝不叠加并发请求
  ——慢网络下请求堆积会拖垮 OJ 服务端与本地渲染。
- **首次执行在一个完整延迟之后**：`start()` 不立即跑 task，首屏数据由调用方自行加载
  （见 `rankStore.startLive` 的注释），避免进入页面先等一个周期。
- **句柄不得进响应式系统**：定时器句柄保存在闭包普通变量中；调用方（store）也把它放在
  模块作用域而非 state —— Vue 深层代理定时器句柄既无意义又可能干扰宿主环境的句柄语义。
- **可测性**：`rand` / `setTimeoutFn` / `clearTimeoutFn` 全部可注入，测试无需 fake timers
  也能精确推进时间。
- 负延迟防御：`jitterMs > intervalMs` 时下界被 `Math.max(0, …)` 钳到 0，避免 setTimeout
  立即触发退化为忙轮询。

## 测试

`src/utils/__tests__/polling.spec.ts`（注入假定时器与随机源）锁定：抖动上下界与钳 0（任意随机值下延迟落在 `[max(0, interval−jitter), interval+jitter]`）、首次执行在一个完整延迟后、任意时刻仅一个待触发句柄、抖动逐周期生效、start/stop 幂等且 stop 后可重启、暂停只跳过本次仍续排、task 同步抛错/异步拒绝都不终止轮询、重入保护与 busy 标志复位。
