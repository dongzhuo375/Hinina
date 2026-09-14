# ProblemSetView（题目总览）

> 源文件：`src/views/ProblemSetView.vue`

## 职责

比赛题目卡片网格页：首屏用比赛题目列表（ac/total）立即渲染，limits / 我的提交状态 / 榜单统计作为补充数据渐进到达；30s±5s 轮询刷新，并处理「比赛未开始」「加载失败」「空列表」三种兜底态。

## 核心类型/函数

常量：`POLL_INTERVAL_MS`(30s)、`POLL_JITTER_MS`(5s)。模块级普通变量：`poller: Poller | null`（副作用句柄，不进 ref/reactive）、`supplementaryLoaded: boolean`（补充数据是否已触发过）。

| 名称 | 签名 | 用途 |
|------|------|------|
| `phase` / `formatLabel` / `startTimeText` | computed | 比赛阶段（`utils/contest`）、赛制文案（OI/ACM）、开赛时间 |
| `showLoading` / `showError` / `showUpcoming` | computed | 三种全屏兜底的判据（见设计要点） |
| `ensureContest` | `() => Promise<void>` | 外壳已拉取则跳过；在途则 `waitLoadingSettled` 等待；否则自己 `loadContest` |
| `loadSupplementary` | `() => void` | 并发拉 limits / myStatus / 榜单第 1 页，**不 await**、各自吞错 |
| `startPolling` / `stopPolling` / `refresh` | — | 30s±5s 轮询：重拉比赛（刷新 ac/total）+ 我的状态（刷新卡片 pill） |
| `openProblem` / `quickSubmit` | `(p: ContestProblem) => void` | 跳转解题页；快捷提交 = 跳转 + `?focus=1`（意图经 query 传递） |

## 直接依赖

- `vue` / `vue-router`
- 组件：`ContestStatsBar` / `ErrorMessage` / `LoadingSpinner` / `ProblemCard`
- stores：`authStore`（uid）/ `contestStore` / `problemStore` / `rankStore`
- `@/types/contest`（仅类型）
- `@/utils/contest`（`getContestPhase`）、`@/utils/polling`（`createPoller` + `Poller` 类型）

## 被依赖

- `router/index.ts` — 路由 `ProblemSet`（`/contest/problems`，Contest 外壳默认重定向目标）

## 逻辑流程

```
onMounted:
  ensureContest()            // 外壳通常已发起；在途则等它落地，避免重复请求
  loadSupplementary()        // limits + myStatus + 榜单（无数据时），并发不 await
  startPolling()             // createPoller(30s±5s, isPaused: document.hidden)

每周期 refresh():
  contestStore.loadContest() 失败 → 本周期跳过（保留旧数据）
  supplementaryLoaded → 只重拉 myStatus（刷新 pill）
  未加载过（挂载时题目为空、开赛后首次出现）→ 补拉 loadSupplementary()

onUnmounted → stopPolling()（回收模块级句柄）
```

设计要点：

- **首屏不等补充数据**：卡片先用比赛题目列表自带的 ac/total 渲染，limits/状态/榜单到达后
  响应式补齐（ProblemCard 对缺失值显示骨架或 `—`）；三个补充请求各自吞错，缺失即缺失。
- **全屏兜底只在无数据时出现**：`showLoading`/`showError` 都要求 `problems.length === 0`——
  已渲染出卡片后，轮询的瞬时失败不应把整页打回错误态/加载态。
- **未开始 ≠ 错误**：`phase === 'upcoming'` 且拿不到题目时显示「比赛尚未开始」提示
  （HOJ 此时通常不放题）；若 OJ 提前放题则照常展示卡片。
- **轮询句柄放模块级普通变量**：副作用句柄而非渲染状态，进 ref/reactive 会被 Vue 深层代理，
  既无意义又可能干扰句柄语义（与 `utils/polling`、`rankStore` 的约定一致）。
- 30s±5s 抖动错峰理由见 `utils/polling` 头注释；切后台暂停（`document.hidden`）。
- 分层约束：View 只经 store 取数，不 import `@/bridge` / `@/services`。
