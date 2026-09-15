# RankView（实时榜单页）

> 源文件：`src/views/RankView.vue`

## 职责

榜单页编排层：引导链（比赛 → 榜单首页 → 开启轮询）、封榜提示、错误分流（有旧数据则非阻断）、页码窗口折叠、每秒时钟驱动「更新于 x 秒前」与阶段/封榜跃迁；表格渲染委托 `ScoreboardTable`，筛选委托 `RankToolbar`。

## 核心类型/函数

模块级/组件级普通变量：`clock`（每秒 setInterval 句柄）、`alive`（卸载标记，异步引导链落地后不得再触碰 store）。

| 名称 | 签名 | 用途 |
|------|------|------|
| `nowMs` / `nowSecs` | ref/computed | 每秒推进的时钟，驱动 updatedText、phase、isSealed |
| `phase` / `isEnded` / `isUpcoming` | computed | `utils/contest.getContestPhase` 派生 |
| `isSealed` | computed | 封榜判据：`sealRank && sealRankTime !== null && nowSecs >= sealRankTime`（只判下界，见设计要点） |
| `isBootstrapping` | computed | 首次加载（无任何可展示数据）才铺满屏 spinner；轮询刷新保留旧表格 |
| `isFatalError` | computed | `error !== null && !hasData` —— 一条数据都没有才整块换 ErrorMessage |
| `showTable` | computed | 底栏（刷新状态 + 分页）只在有表格可解释时出现 |
| `updatedText` / `liveText` | computed | 「更新于 x 秒/分/小时前」；「比赛已结束，停止刷新」/「实时刷新中」 |
| `pageItems` | computed | 页码窗口：首尾页 + 当前页 ±1，其余折叠 `…`（≤7 页全量展示） |
| `ensureContest` | `() => Promise<void>` | 子视图 onMounted 先于外壳触发，直达本页时这里才是实际发起方；外壳在途则每 100ms 轮询等待（上限 100 次，且 `alive` 短路） |
| `bootstrap` / `retry` / `goToPage` | — | 引导链 / 错误条与致命错误重试 / 翻页（失败静默，旧数据保留） |

## 直接依赖

- `vue`
- 组件：`ContestStatsBar` / `ErrorMessage` / `LoadingSpinner` / `RankToolbar` / `ScoreboardTable`
- stores：`authStore`（uid）/ `contestStore` / `rankStore`
- `@/utils/contest`（`getContestPhase`）

## 被依赖

- `router/index.ts` — 路由 `Rank`（`/contest/rank`）

## 逻辑流程

```
onMounted: clock = setInterval(1s)；bootstrap()

bootstrap():
  ensureContest() → 无 contestId 则终止
  rankStore.loadRank(contestId, uid, 1)      // 失败原因已写入 store.error
  !isEnded → rankStore.startLive(() => isEnded.value)
              // 已结束不启动：HOJ 网页端此时禁用刷新，继续打请求只是浪费服务端算力
              // 暂停判据传入 isEnded：停留期间比赛结束由 watch 兜底 stopLive

onUnmounted: alive = false；clearInterval(clock)；rankStore.stopLive()
              // 轮询器是模块级副作用句柄，离开榜单页必须回收，否则后台持续打 OJ

watch(isEnded): 结束瞬间立即 stopLive()
```

设计要点：

- **封榜判定只看 `contest` 字段，不能依赖 `forceRefresh` 的返回**（HOJ 文档 §9.3）：
  非创建者/超管传 `forceRefresh: true` 会被服务端静默忽略，用它反推封榜永远得不到真值。
  文档 §4 把封榜窗口写作 `[sealRankTime, endTime]`，但结束后服务端**不会自动解封**
  （需管理员手动解除），故只判下界——避免结束后提示消失、选手误以为榜单已解封。
- **错误分流**：`isFatalError`（无数据）→ 整块 ErrorMessage；`error && hasData` →
  顶部一条非阻断错误条（「已保留上一次数据，将自动重试」+ 立即重试按钮），绝不清空榜单。
- **引导链竞态**：子视图 `onMounted` 先于父外壳触发，直达榜单页时本视图是比赛数据的
  实际发起方；外壳已在途则等待其落地（`contestStore.loadContest` 本身也有 in-flight 去重，
  双保险）。`alive` 标记防止卸载后的异步回调再写 store。
- 页码窗口折叠：50 条/页时几百人的比赛有十几页，全量页码会挤爆底栏。
- 每秒时钟同时驱动三件事（更新时间文案、阶段跃迁、封榜判定），单一时间源避免漂移。
