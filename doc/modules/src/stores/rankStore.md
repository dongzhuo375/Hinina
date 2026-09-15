# rankStore（榜单状态与实时刷新编排）

> 源文件：`src/stores/rankStore.ts`

## 职责

持有当前比赛的榜单分页数据与查询条件，编排 10s±2s 的实时刷新轮询，并为打星/女生筛选维护**全量快照模式**（跨页拉取 + 客户端过滤分页，轮询暂停）；入页数据的服务端怪癖归一（uid 去重、我的行定位、参与人数修正）统一在 `applyPage` 完成，视图层不再重复处理。

## 核心类型/函数

常量：`RANK_POLL_INTERVAL_MS`(10s)、`RANK_POLL_JITTER_MS`(2s) —— HOJ 文档 §9.9 要求间隔 ≥10s 且切后台暂停，抖动打散全场客户端的同步相位；`FULL_FETCH_MAX_PAGES`(40)、`FULL_FETCH_MAX_ROWS`(2000) —— 全量快照拉取上限（40 页 × 50 行），超限截断并由 UI 明示「结果可能不完整」，避免赛场上无限拉页打爆 OJ。

| 名称 | 签名 | 用途 |
|------|------|------|
| `RankGroupFilter` | `'all' \| 'official' \| 'star' \| 'female'` | 分组筛选（类型与过滤纯函数定义在 `utils/rank`，此处 re-export）；official 走服务端 `removeStar`，star/female 服务端无对应参数 → 全量快照模式 |
| `FullFetchState` | `'idle' \| 'loading' \| 'done' \| 'truncated' \| 'error'` | 快照拉取状态：未拉取 / 拉取中 / 完整 / 达上限截断 / 失败（无可用快照） |
| `poller` | 模块级 `Poller \| null` | 轮询器句柄（副作用句柄而非渲染状态，**不进响应式系统**） |
| `isPageHidden` / `isFullSnapshotFilter` | 模块内私有 fn | `document.hidden` 判后台；`filter ∈ {star, female}` 判是否需要全量快照 |
| state | `rows`（已去重）/ `myRow` / `total` / `size` / `current` / `pages` / `participants` / `keyword` / `removeStar` / `groupFilter` / `isLoading` / `isLive` / `error` / `lastUpdated` / `contestId` / `uid` | `contestId`/`uid` 是轮询上下文，供 `refresh` 复用，登出随 `$reset` 清理 |
| state（全量快照） | `fullRows: ContestRankRow[] \| null` / `fullFetchState` / `fullPage` / `fullLoadedRows` | 跨页去重合并的快照（非全量模式为 null）/ 拉取状态 / 客户端分页当前页 / 已加载行数（状态行「已加载 N 行」用） |
| `isFullMode` | getter | 是否处于全量快照模式（star/female 筛选**且快照已就位**） |
| `fullFilteredRows` | getter | 快照按当前分组过滤后的全部行（未分页，`filterRankRowsByGroup`） |
| `visibleRows` | getter | 全量模式：过滤快照后 `paginateRankRows` 客户端切片（`pages`/`current` 已由 `syncFullPaging` 同步为等效值，视图无需感知模式差异）；常规模式：`filterRankRowsByGroup(rows, groupFilter)`——star/female 但快照未就位（拉取中/失败）时退化为筛当前页，聊胜于无 |
| `hasData` | getter | 是否已加载过（区分「空榜单」与「尚未请求」） |
| `loadRank` | `(contestId, uid, page?) => Promise<void>` | 加载一页；失败记录 `error` 并**向上抛出**，由调用方决定是否提示 |
| `applyPage` | `(page, uid) => void` | 应用一页数据的三处归一（见逻辑流程） |
| `fetchAllRows` | `() => Promise<void>` | 全量快照拉取，见逻辑流程 |
| `syncFullPaging` | `() => void` | 把客户端分页状态同步到 `pages`/`current`（RankView 分页条直接读这两个字段） |
| `refresh` | `() => Promise<void>` | 轮询/手动刷新：**吞掉异常**；全量筛选态下重拉整个快照（只会被手动刷新触发），否则重拉当前页 |
| `setPage` | `(page) => Promise<void>` | 翻页（越界忽略）；全量模式为**纯客户端切片，不发请求** |
| `setKeyword` | `(keyword) => Promise<void>` | 搜索（trim 后回到第 1 页）；全量模式下 keyword 是快照拉取的请求参数，须**重拉快照** |
| `setGroupFilter` | `(filter) => Promise<void>` | 分组切换，见逻辑流程 |
| `startLive` | `(isPaused?: () => boolean) => void` | 开启实时刷新；先 `stopLive()` 防重复；暂停判据 = 页面隐藏 ‖ **全量快照模式** ‖ 调用方条件 |
| `stopLive` | `() => void` | 停止轮询并置空句柄（离开榜单页、比赛结束、登出时调用） |

## 直接依赖

- `pinia`
- `@/types/rank`（仅类型）
- `@/services/rank.service`（`rankService.getRank` + `DEFAULT_RANK_PAGE_SIZE`）
- `@/utils/polling`（`createPoller` + `Poller` 类型）
- `@/utils/rank`（`dedupeRankRows` / `filterRankRowsByGroup` / `mergeRankPages` / `paginateRankRows` / `resolveMyRow` / `resolveParticipantCountFromPage` + `RankGroupFilter` 类型）

## 被依赖

- `views/RankView.vue` — 榜单页：加载与实时刷新编排（`loadRank` / `setPage` / `startLive`（比赛结束作为暂停判据）/ `stopLive`）、错误重试与手动刷新入口（`refresh`）、全量快照模式状态行（`isFullMode` / `fullFetchState` / `fullLoadedRows` / `fullFilteredRows`）
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
      3. participants = resolveParticipantCountFromPage(page, uid)
                                               // total − 本页重复行数 − 我的窗外前置副本
      4. lastUpdated = Date.now()
  失败 → error 记录 + rethrow（401 已由全局会话守卫统一处理，此处不重复应对）

setGroupFilter(filter)
  star/female（全量快照筛选）→ removeStar = false、fullPage = 1 → fetchAllRows()
                               // star↔female 互切也重拉以获得新数据
  all/official → 清空快照（fullRows/fullFetchState/fullPage/fullLoadedRows 复位）
                 official 置 removeStar；removeStar 未变且**非从全量模式退出**才短路
                 // 退出全量模式时 pages/current 已被客户端分页覆写，必须重载第 1 页

fetchAllRows()
  无 contestId 或已在 loading → 短路（重入保护）
  从第 1 页起顺序 getRank({ keyword, removeStar: false, limit: 50 })
    // removeStar 恒 false：快照必须包含打星行，否则「打星队」筛选恒为空
    // keyword 保持生效：每页请求都带上
  merged = mergeRankPages(merged, records)     // 每页都会前置复制我/关注用户，跨页按 uid 去重
  page >= pages → done；page >= 40 或 merged >= 2000 行 → truncated
  成功 → fullRows/fullLoadedRows/fullFetchState/lastUpdated + syncFullPaging()
  失败 → 有旧快照：保留快照并回退到拉取前状态（不能停在 loading，否则重入保护永久锁死）；
         毫无快照：fullFetchState = 'error'；error 记录 + rethrow

syncFullPaging()
  pages = max(1, ceil(fullFilteredRows.length / 50))；fullPage 钳位；current = fullPage
  // RankView 分页条直接读 pages/current，同步等效值后视图无需感知模式差异

startLive(isPaused?)
  → stopLive()（幂等防重复定时器）
  → poller = createPoller({
      task: refresh,                            // 吞错，不打断轮询节奏；全量筛选态下
                                                // 重拉整个快照（只会被手动刷新触发）
      intervalMs: 10s, jitterMs: 2s,
      isPaused: 页面隐藏 || 全量快照模式（star/female） || 调用方条件（如比赛已结束）,
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
  `resolveParticipantCountFromPage(page, uid)` = `total − 本页重复行数 − 我的窗外前置副本`
  修正（口径与残余误差详见 `utils/rank.md`），视图层不再各自推算。
- `setKeyword` 后服务端会在**全量排名**上重新过滤再分页，`total` 随关键词变化（HOJ §9.4），
  因此必须回到第 1 页。
- **分组筛选的服务端/客户端分流**：`official` 走服务端参数（removeStar）重新请求；
  star/female 服务端无对应参数，只筛当前页会**跨页漏行**（打星队/女生队散布在各页），
  故进入全量快照模式——一次拉全（有 40 页/2000 行上限）后纯客户端过滤 + 切片分页。
- **全量模式下自动轮询暂停**：整榜重拉太重，不能进 10s 轮询；暂停判据直接写在
  `startLive` 的 `isPaused` 里（轮询器保持存活，切回 all/official 自动恢复），
  刷新只保留 RankView 状态行的「手动刷新」入口（经 `refresh` → `fetchAllRows`）。
- **快照失败保留旧数据**：重拉失败且有旧快照时回退到拉取前状态继续浏览（状态不能停在
  `loading`，否则重入保护会把 `fetchAllRows` 永久锁死）；毫无快照才标记 `error`，
  此时 `visibleRows` 退化为筛当前页——数据不完整也好过整页空白。

## 测试

`src/stores/__tests__/rankStore.spec.ts`（mock rankService，31 例）锁定：

- **applyPage 三处归一**：uid 去重、我的行从未去重 records 定位（uid 为空不抛错）、参与人数 `resolveParticipantCountFromPage(page, uid)` 修正、分页信息与刷新时间记录。
- **loadRank / refresh**：失败记录原因、复位 loading 并抛出但**保留上一次数据**；`refresh` 吞异常且无 contestId 不发请求。
- **分页与筛选**：`setPage` 越界短路 / 范围内按页请求；`setKeyword` trim 后回第 1 页且空串传 null；official 置 removeStar 重新请求（服务端参数）、从 official 切回 all 复位重载。
- **全量快照模式专节**：`setGroupFilter(star)` 顺序拉取全部页并按 uid 跨页去重合并；star↔female 互切重拉快照并按新分组过滤；全量模式翻页是纯客户端切片不发请求；超过 40 页上限截断并标记 `truncated`；全量模式自动轮询暂停、`refresh()` 手动触发重拉快照；切回 all/official 清空快照恢复服务端分页；全量模式 `setKeyword` 携带关键词重拉快照并回第 1 页；快照失败且无旧快照标记 `error`（`visibleRows` 退化为筛当前页）；重拉失败保留旧快照继续浏览（状态不回退、不停在 loading）；无 contestId 不发请求。
- **startLive / stopLive**：按 10s±2s 抖动刷新、停止后不再刷新、重复调用不产生多个轮询器、暂停判据生效时跳过请求但轮询存活、`stopLive` 幂等。
