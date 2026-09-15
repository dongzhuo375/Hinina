# RankView（实时榜单页）

> 源文件：`src/views/RankView.vue`

## 职责

榜单页编排层：引导链（比赛 → 榜单首页 → 开启轮询）、封榜提示、错误分流（有旧数据则非阻断）、全量快照模式状态行（打星/女生客户端筛选）、页码窗口折叠、每秒时钟驱动「更新于 x 秒前」与阶段/封榜跃迁；表格渲染委托 `ScoreboardTable`，筛选委托 `RankToolbar`。

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
| `updatedText` / `liveText` | computed | 「更新于 x 秒/分/小时前」；刷新状态文案三分支：已结束 →「比赛已结束，停止刷新」，全量快照模式 →「全量快照 · 手动刷新」，否则「实时刷新中 / 未开启」（底栏呼吸灯同步：`isLive && !isFullMode` 才绿色脉冲，否则琥珀） |
| `isFullFilterActive` | computed | 是否处于打星/女生的客户端筛选（**含快照尚未就位**的拉取中/失败态，比 `rankStore.isFullMode` 宽） |
| `totalLabel` | computed | 底栏「共 N 队」：全量模式显示筛选后行数（`fullFilteredRows.length`，标注「筛选后」），常规模式显示修正后的真实参与人数 |
| `pageItems` | computed | 页码窗口：首尾页 + 当前页 ±1，其余折叠 `…`（≤7 页全量展示） |
| `ensureContest` | `() => Promise<void>` | 子视图 onMounted 先于外壳触发，直达本页时这里才是实际发起方；统一走 `contestStore.whenLoaded()`（P59：复用在途请求，原 100ms sleep 轮询写法已删除） |
| `bootstrap` / `retry` / `manualRefresh` / `goToPage` | — | 引导链 / 错误条与致命错误重试（全量筛选态下错误来自快照拉取，重试走 `rankStore.refresh()` 重拉快照）/ 手动刷新按钮（`refresh` 内部按模式分流且吞异常）/ 翻页（失败静默，旧数据保留；全量模式为纯客户端切片） |

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

全量快照模式状态行（`isFullFilterActive` 时显示在错误条之下、表格之上）：

```
快照就位（isFullMode）→「客户端筛选 · 已加载 {fullLoadedRows} 行」
  fullFetchState === 'truncated' → 追加「（已达 2000 行上限，结果可能不完整）」
loading →「正在拉取全量榜单…」；error →「拉取失败，当前仅筛选本页数据」
右侧「手动刷新」按钮 → manualRefresh()（全量模式下自动轮询已暂停，这是唯一刷新入口）
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
- **全量快照模式（打星/女生筛选）下视图只做「明示」**：跨页拉取与客户端分页归
  `rankStore`（`syncFullPaging` 已把 `pages`/`current` 同步为等效值，本视图的分页条
  无需感知模式差异）；视图负责的是自动轮询暂停后的可解释性——状态行（快照规模/
  截断/失败 + 手动刷新入口）、`liveText`、底栏计数与呼吸灯都切换到全量口径。
- 每秒时钟同时驱动三件事（更新时间文案、阶段跃迁、封榜判定），单一时间源避免漂移。
