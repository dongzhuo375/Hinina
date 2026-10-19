# brief-retry（匿名简报重试延迟）

> 源文件：`src/utils/brief-retry.ts`

## 职责

给出登录页匿名比赛简报加载失败后的**有界**指数退避延迟表：把「第几次重试等多久」这条策略抽成纯函数，登录页只负责按返回值排程。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `BRIEF_RETRY_DELAYS_MS` | `readonly [2000, 4000, 8000]` | 三次重试的延迟表（2s → 4s → 8s） |
| `briefRetryDelayMs` | `(retriesAlreadyUsed: number) => number \| null` | 已重试 N 次时的下次延迟；用尽或非法入参返回 `null`（= 不再重试） |

## 直接依赖

无（零 import 的纯模块）

## 被依赖

- `views/LoginView.vue` — 简报加载失败后的重试排程（`null` 即停止重试并保留「网络异常」提示）

## 逻辑流程

```
briefRetryDelayMs(n):
  n 非整数或 n < 0        → null
  BRIEF_RETRY_DELAYS_MS[n] ?? null
    n = 0 → 2000
    n = 1 → 4000
    n = 2 → 8000
    n ≥ 3 → null（重试预算用尽）
```

设计要点：

- **必须有界**：登录页是应用入口，无限重试会在 OJ 不可达时持续打请求；3 次（合计约 14s）足够覆盖瞬时抖动，之后把「网络异常 + 手动重试」交给用户。
- **退避而非固定间隔**：失败往往是服务端不可用而非网络丢包，等长间隔只是重复打同一条路。
- **策略与执行分离**：本模块只回答「等多久」，排程/清理/计数在 LoginView（`briefRetryCount` + `briefRetryTimer`），便于单测覆盖边界而不启动组件。

## 测试

`src/utils/__tests__/brief-retry.spec.ts` 锁定：`[0,1,2]` 依次映射为 `[2000,4000,8000]`；`3`（用尽）、`-1`（负数）、`1.5`（非整数）一律返回 `null`。
