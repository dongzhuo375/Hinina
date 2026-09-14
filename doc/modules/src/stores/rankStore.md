# rankStore（榜单状态与实时刷新编排）

> 源文件：`src/stores/rankStore.ts`

## 职责

持有当前比赛的榜单分页数据与查询条件，并编排 10s±2s 的实时刷新轮询；入页数据的服务端怪癖归一（uid 去重、我的行定位、参与人数修正）统一在 `applyPage` 完成，视图层不再重复处理。

## 核心类型/函数

常量：`RANK_POLL_INTERVAL_MS`(10s)、`RANK_POLL_JITTER_MS`(2s) —— HOJ 文档 §9.9 要求间隔 ≥10s 且切后台暂停，抖动打散全场客户端的同步相位。

| 名称 | 签名 | 用途 |
|------|------|------|
| `RankGroupFilter` | `'all' \| 'official' \| 'star' \| 'female'` | 分组筛选；official 走服务端 `removeStar`，star/female 服务端无对应参数故客户端过滤 |
| `poller` | 模块级 `Poller \| null` | 轮询器句柄（副作用句柄而非渲染状态，**不进响应式系统**） |
| `isPageHidden` | `() => boolean`（模块内私有） | `document.hidden`，窗口最小化/切后台时暂停轮询 |
| state | `rows`（已去重）/ `myRow` / `total` / `size` / `current` / `pages` / `participants` / `keyword` / `removeStar` / `groupFilter` / `isLoading` / `isLive` / `error` / `lastUpdated` / `contestId` / `uid` | `contestId`/`uid` 是轮询上下文，供 `refresh` 复用，登出随 `$reset` 清理 |
| `visibleRows` | getter | 按 `groupFilter` 过滤后的行（star: `rank === -1`；female: `gender === 'female'`） |
| `hasData` | getter | 是否已加载过（区分「空榜单」与「尚未请求」） |
| `loadRank` | `(contestId, uid, page?) => Promise<void>` | 加载一页；失败记录 `error` 并**向上抛出**，由调用方决定是否提示 |
| `applyPage` | `(page, uid) => void` | 应用一页数据的三处归一（见逻辑流程） |
| `refresh` | `() => Promise<void>` | 轮询用刷新：**吞掉异常**，瞬时失败保留上一次数据等下一周期 |
| `setPage` / `setKeyword` / `setGroupFilter` | actions | 翻页（越界忽略）/ 搜索（trim 后回到第 1 页）/ 分组切换 |
| `startLive` | `(isPaused?: () => boolean) => void` | 开启实时刷新；先 `stopLive()` 防重复 |
| `stopLive` | `() => void` | 停止轮询并置空句柄（离开榜单页、比赛结束、登出时调用） |

## 直接依赖

- `pinia`
- `@/types/rank`（仅类型）
- `@/services/rank.service`（`rankService.getRank`）
- `@/utils/polling`（`createPoller` + `Poller` 类型）
- `@/utils/rank`（`dedupeRankRows` / `resolveMyRow` / `resolveParticipantCountFromPage`）

## 被依赖

- `views/RankView.vue` — 榜单页：加载与实时刷新编排（`loadRank` / `setPage` / `startLive`（比赛结束作为暂停判据）/ `stopLive`）与错误重试入口
- `views/ProblemSetView.vue` — 无榜单数据时拉一次 `loadRank`，驱动统计卡与卡片状态
- `components/rank/ScoreboardTable.vue` — 表格渲染（`visibleRows` 与「我的行」高亮）
- `components/rank/RankToolbar.vue` — 搜索（`setKeyword`）与分组筛选（`setGroupFilter`）动作（含 `RankGroupFilter` 类型）
- `components/contest/ContestStatsBar.vue` — 统计卡读 `myRow` / `participants` / `lastUpdated`
- `components/problem/ProblemCard.vue` — 读 `myRow.submissionInfo` 展示该题我的通过/尝试情况
- `stores/session.ts` — 登出清理：`stopLive()` + `$reset()`
- `stores/__tests__/rankStore.spec.ts` — 单元测试（mock rankService）

## 逻辑流程

```
loadRank(contestId, uid, page)
  → rankService.getRank({ contestId, currentPage, keyword, removeStar })
  → applyPage(result, uid)：
      1. rows = dedupeRankRows(records)        // 服务端把当前用户/关注用户前置复制了一份
      2. myRow = resolveMyRow(原始 records, uid) // 从未去重的 records 定位——前置副本正是它
                                               // 存在的意义，任意页都能取到「我的行」
      3. participants = resolveParticipantCountFromPage(page) // total − 本页重复行数
      4. lastUpdated = Date.now()
  失败 → error 记录 + rethrow（401 已由全局会话守卫统一处理，此处不重复应对）

startLive(isPaused?)
  → stopLive()（幂等防重复定时器）
  → poller = createPoller({
      task: refresh,                            // 吞错，不打断轮询节奏
      intervalMs: 10s, jitterMs: 2s,
      isPaused: 页面隐藏 || 调用方条件（如比赛已结束）,
      onError: 记录 error })
  → poller.start()；首次执行在一个完整周期后 —— 首屏数据由调用方自行 loadRank，
    进入页面立刻可见而不是等 10 秒

stopLive → poller?.stop() + 置 null + isLive = false
```

设计要点：

- **轮询编排归 store 而非组件**：榜单数据被多个视图共享（榜单页、题目总览、统计卡），
  组件级 onUnmounted 停表在视图切换/keep-alive 场景下不可靠；store 与会话同生命周期，
  登出时由 `stores/session` 统一 `stopLive()` + `$reset()`（含「我的行」，属会话数据）。
- **句柄放模块作用域**：`poller` 是副作用句柄，不进 state —— Vue 深层代理定时器句柄
  无意义且可能干扰句柄语义（与 `utils/polling` 的约定一致）。
- **失败保留旧数据**：赛场网络抖动频繁，`refresh` 吞错让表格停留在上一次成功快照，
  比闪成空态/错误态更可用；错误仍写入 `error` 供 UI 提示。
- **参与人数不用分页 `total` 原值**：服务端把当前用户/关注用户前置复制进 records，
  `total` 随之偏大（HOJ §9.2）；`applyPage` 统一改用
  `resolveParticipantCountFromPage(page)` = `total − 本页重复行数` 修正（口径与残余误差
  详见 `utils/rank.md`），视图层不再各自推算。
- `setKeyword` 后服务端会在**全量排名**上重新过滤再分页，`total` 随关键词变化（HOJ §9.4），
  因此必须回到第 1 页。
- `setGroupFilter` 只有 `official` 触发重新请求（removeStar 是服务端参数）；
  star/female 为纯客户端过滤，切换零网络开销。

## 测试

`src/stores/__tests__/rankStore.spec.ts`（mock rankService）锁定：`applyPage` 三处归一（uid 去重、我的行从未去重 records 定位、参与人数 total−重复行修正）、`loadRank` 失败记录原因并抛出但**保留上一次数据**、`refresh` 吞异常且无 contestId 不发请求、`setPage` 越界短路、`setKeyword` trim 后回第 1 页且空串传 null、official/星/女生筛选的服务端与客户端分流、`startLive` 按 10s±2s 抖动刷新且重复调用不产生多个轮询器、暂停判据生效时跳过请求但轮询存活、`stopLive` 幂等。
