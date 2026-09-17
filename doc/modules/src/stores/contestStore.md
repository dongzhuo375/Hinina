# contestStore（比赛状态与匿名简报）

> 源文件：`src/stores/contestStore.ts`

## 职责

持有当前比赛实体与题目列表（会话相关），以及登录页的匿名比赛简报（与会话无关），并为「比赛数据加载」提供并发去重，避免开赛瞬间重复请求放大 OJ 服务端压力。

## 核心类型/函数

模块级 `loadInFlight: Promise<void> | null` — 进行中的加载请求（副作用句柄，**不进响应式状态**）。

**状态分两类，清理策略不同**：

| 类别 | 字段 | 说明 |
|------|------|------|
| 会话相关 | `contest` / `problems` / `isLoading` / `error` | 登录后加载的配置比赛数据，登出必须清空 |
| 登录页匿名简报 | `brief` / `briefBaseUrl` / `briefState` / `briefError` | 不依赖会话，登出后**保留** |

| 名称 | 签名 | 用途 |
|------|------|------|
| `currentProblemIds` | getter | 当前比赛所有题目的 displayId 列表 |
| `problemCount` | getter | 比赛题目数量 |
| `whenLoaded` | `() => Promise<void>` | **等待比赛数据就绪的统一入口（P59）**：已有数据立即返回；加载在途复用同一 in-flight Promise；无人加载则由本调用发起。失败 rejection 透传（错误已写入 `error`），调用方自行 catch |
| `loadContest` | `() => Promise<void>` | 加载配置比赛（并发去重，见逻辑流程）；失败写入 `error` 并抛出 |
| `loadBrief` | `() => Promise<void>` | 加载匿名简报；三态映射，**失败不抛出** |
| `clearSessionData` | `() => void` | 只清空会话相关四项，保留匿名简报 |

`briefState`：`'idle' | 'connecting' | 'connected' | 'failed' | 'unconfigured'`。

## 直接依赖

- `pinia`
- `@/types/contest`（仅类型）
- `@/services/contest.service`（`loadConfiguredContest` / `loadContestBrief`）
- `@/utils/error`（`errorMessage` —— 错误文案收敛）

## 被依赖

- `views/ContestLayout.vue` — 外壳挂载时经 `whenLoaded()` 触发加载，就绪后启动公告轮询
- `views/RankView.vue` / `ProblemSetView.vue` / `ProblemSolveView.vue` / `AnnouncementsView.vue` / `SubmissionsView.vue` — 比赛数据读取与 `whenLoaded()` 统一等待入口（P59：三视图各自的 sleep 轮询 / watch 写法已删除）
- `views/LoginView.vue` — 匿名简报（`brief` / `briefState` / `briefError` / `briefBaseUrl`，驱动右侧氛围区与重试入口）
- `components/contest/ContestStatsBar.vue`（赛制与题目数）、`components/rank/ScoreboardTable.vue` / `RankToolbar.vue`（赛制、rankShowName、题目列头）、`components/layout/TopBar.vue` / `StatusBar.vue`（标题、倒计时、连接状态）、`components/problem/ProblemTabStrip.vue` / `ProblemStatement.vue`（题目列表）
- `stores/session.ts` — 登出清理走 `clearSessionData()` 而非 `$reset()`

## 逻辑流程

**loadContest() —— 并发去重契约**：

```
loadInFlight 非空 → 直接返回同一个 in-flight Promise（不发第二次 IPC）
否则：
  isLoading = true, error = null
  task = (async () => {
    contestService.loadConfiguredContest() → 填充 contest / problems
    失败 → error 记录 + rethrow        // 所有并发调用方收到同一个 rejection，
                                       // 因此每个调用方必须自行 catch
    finally → isLoading = false
              loadInFlight = null      // 必须清理：否则失败后再也发不出请求
  })()
  loadInFlight = task; return task
```

为什么需要：外壳 `ContestLayout` 与题目总览/榜单/解题三个视图可能在同一时刻各自发现
「比赛数据未加载」而触发加载；开赛瞬间全场客户端同时进场时，重复请求会成倍放大 OJ
服务端压力。共享 in-flight Promise 把 N 个并发调用收敛为 1 次 IPC。

**loadBrief() —— 三态**：

```
briefState = 'connecting'
→ contestService.loadContestBrief()
   ├─ status === 'unconfigured'（config 未设置 contestId）→ briefState = 'unconfigured'
   │    // 这是配置缺失的提示态，不是错误，不写 briefError
   ├─ status === 'ok' → brief = result.contest（ID 不在列表中时为 null）
   │                    briefBaseUrl = result.baseUrl; briefState = 'connected'
   └─ 抛异常 → brief = null, briefError = 原因, briefState = 'failed'（不抛出，
        登录页不应因简报失败而中断，由 briefState/briefError 驱动连接状态与重试入口）
```

设计要点：

- **`clearSessionData()` 而非 `$reset()`**：匿名简报与登录态无关，切换账号时若被清空，
  登录页右侧氛围区与倒计时会瞬间空白（详见 `doc/modules/src/stores/session.md`）。
- `loadInFlight` 放模块作用域而非 state：Promise 句柄进响应式系统既无意义又有代理开销，
  且 `$reset()` / `clearSessionData()` 都清不掉它，回收靠 `finally` 自清。

## 测试

`src/stores/__tests__/contestStore.spec.ts` 锁定：并发调用共享同一请求、失败时两个调用方都收到 rejection 且 `loadInFlight` 被清理（允许后续重试）、`loadBrief` 三态（unconfigured 不算 failed、失败不抛出）、`clearSessionData` 清空会话状态但保留匿名简报、`whenLoaded` 四态（已有数据零请求 / 在途复用 / 无人加载时发起 / 失败透传）。
