# announcementStore（公告列表与已读状态）

> 源文件：`src/stores/announcementStore.ts`

## 职责

比赛公告列表 + 客户端已读状态（HOJ 无已读概念，Rust 端按「比赛 + 用户」本地持久化）。为 ActivityBar 红点提供 `unreadCount`，为公告页提供列表与 `markAllRead`。已读语义由 `isWatching`（用户是否正看着公告页）与 `document.hidden` 共同界定 —— **只有「在屏幕上」才等于「看过」**。

## 核心类型/函数

模块级 `poller: Poller | null` — 轮询器句柄放模块作用域（不进响应式系统），由 `stopLive` 回收；登出经 `stores/session` 统一停止。

模块级 `visibilityRefresh: (() => void) | null` + `lastVisibilityRefreshAt: number` + `VISIBILITY_REFRESH_DEDUPE_MS = 1000` — 「窗口重新可见/聚焦 → 立即补拉」的监听句柄与去重时间戳，与 `poller` 同款「副作用句柄不进响应式系统」约定，由 `installVisibilityRefresh` / `removeVisibilityRefresh` 成对管理。

| 名称 | 签名 | 用途 |
|------|------|------|
| state | `announcements` / `readIds` / `total` / `isLoading` / `isLive` / `error` / `contestId` / `isWatching` | 列表（服务端顺序原样保留）、已读 ID 数组、请求状态、轮询上下文、「用户正在看公告页」标记（由 `AnnouncementsView` 挂载/卸载维护） |
| `unreadCount` | getter `=> number` | 未读数（ActivityBar 红点数据源） |
| `isUnread` | getter `(id) => boolean` | 单条未读判定（列表圆点） |
| `hasData` | getter `=> boolean` | 区分「空列表」与「尚未请求」 |
| `load` | `(contestId) => Promise<void>` | 并行拉列表 + 已读集合；**已读拉取失败降级为空集**（宁可多显红点，不可漏报公告）；列表失败写 error 并抛出；`isWatching && !document.hidden` 时顺带 `markAllRead()` —— 「加载后标记」与「刷新后标记」由此收敛为**一条**路径 |
| `refresh` | `() => Promise<void>` | 轮询用刷新：吞掉异常，不打断轮询节奏（也使其可安全用作可见性补拉） |
| `markAllRead` | `() => Promise<void>` | **页面隐藏时直接返回**（切走了 = 没看到，标记已读等于把红点吞掉）；否则乐观更新本地 → 后端持久化 → 以后端合并结果为准；**持久化失败回滚本地**（红点复发优于假已读） |
| `startLive` | `(contestId, isPaused?) => void` | 立即拉取一次（红点不等轮询周期）+ 60s±10s createPoller（`document.hidden` 暂停）+ 注册可见性补拉监听（`visibilitychange` / `focus`，1s 去重） |
| `stopLive` | `() => void` | 停止轮询 + 注销可见性监听并复位去重时间戳（离开工作台/比赛结束/登出） |

## 直接依赖

- `pinia`
- `@/types/announcement`（仅类型）
- `@/services/announcement.service`
- `@/utils/polling`（`createPoller`）
- `@/utils/error`（`errorMessage` —— 错误文案收敛）
- `@/utils/logger`（`createLogger` —— 作用域日志）

## 被依赖

- `views/ContestLayout.vue` — 外壳启动/停止轮询（红点在全部页面保持鲜活）
- `views/AnnouncementsView.vue` — 列表渲染 + 挂载置 `isWatching = true`、卸载置回 `false`（`load` 据此决定是否标记已读）
- `components/layout/ActivityBar.vue` — `unreadCount` 红点徽标
- `stores/session.ts` — 登出清理：`stopLive()` + `$reset()`
- `main.ts` — 组合根订阅新公告事件（`installAnnouncementListener`），`contestId` 匹配时调 `refresh()`

## 逻辑流程

```
ContestLayout（比赛就绪）→ startLive(cid, 比赛已结束判据)
  ├─ refresh() 立即一次
  ├─ createPoller(60s±10s, hidden 暂停) → refresh()
  └─ installVisibilityRefresh → visibilitychange / focus → refresh()（1s 内只补一次）
     └─ stopLive() → poller.stop() + removeVisibilityRefresh()（复位去重时间戳）

新公告事件（Rust 基线比对 → announcements-published）
  → main.ts installAnnouncementListener → contestId 匹配 → refresh()

AnnouncementsView onMounted
  → isWatching = true（**先于 bootstrap**：load 要据此标记已读）
  → contestStore.whenLoaded() → load(contestId)
     ├─ 并行：listAnnouncements + getReadIds（后者失败降级空集）
     ├─ 落地列表/总数/已读集合
     └─ isWatching && !document.hidden → markAllRead()
        ├─ 页面隐藏 → 直接返回（没看到 = 不标记）
        ├─ 本地乐观：readIds += 未读 ids（红点立即消失）
        ├─ service.markRead → 后端合并集合 → 替换 readIds
        └─ 失败 → 回滚 readIds + log.error 记录（utils/logger 作用域日志）

AnnouncementsView onUnmounted → isWatching = false（此后到达的新公告保持未读，红点才会亮）
```

设计要点：

- **未读语义（产品决策）**：红点 = `unreadCount > 0`；**看着公告页时**列表即已读；已读按「比赛+用户」落盘，重启/重登不复发。
- **`isWatching` 解决「假红点」**：没有它，用户停留在公告页期间到达的新公告会一直挂着未读 —— 离开页面后突然冒出一个「新公告」红点，而内容其实早就看过了。反过来，`markAllRead` 在页面隐藏时短路，解决「真红点被吞」：切走了并没看到，标记已读等于让选手永远错过这条公告。
- **可见性补拉（`installVisibilityRefresh`）**：桌面客户端的常态是「切出去看题解/记笔记，再切回来」，而 `document.hidden` 由 true 变回 false 时轮询器只是恢复排程、**不会补发一次** —— 只靠 60s 节拍意味着切回来最多要等 70s 才可能看到红点，实测被选手直接感知为「红点不出现，必须手动刷新页面」。补拉是幂等的（`refresh` 内部吞错），且不改变 60s 的稳态节拍。
- **1s 去重（`VISIBILITY_REFRESH_DEDUPE_MS`）**：`visibilitychange` 与 `focus` 常成对触发，一次切回打两次请求纯属浪费；去重时间戳在 `removeVisibilityRefresh` 里复位，避免模块级状态跨 `startLive`/测试泄漏（否则下一轮的第一次可见性事件会被上一轮的时间戳挡住）。
- **标记已读只有一条路径**：`markAllRead` 由 `load` 在 `isWatching` 时统一触发，视图不再单独调用 —— 否则「加载后标记」与「刷新后标记」两条路径各自维护，必然漂移（其中一条漏了 `isWatching` 判断就会吞红点）。
- 轮询节奏 60s±10s：公告由裁判组低频发布，无需榜单级实时性；抖动打散全场客户端相位。
- 已读集合用数组承载（保留响应式），判定走 getter 内部 `Set`。

## 测试

`src/stores/__tests__/announcementStore.spec.ts`：load 成功/已读降级/列表失败、unreadCount 与 isUnread、markAllRead 乐观更新+失败回滚+无未读短路+空 contestId 静默+**页面不可见不标记**、**isWatching（正在看时 load 落地即已读 / 不在页面不标记 / 正在看但页面不可见不标记）**、refresh 吞错、startLive 立即拉取+周期刷新+stopLive+重复 startLive 幂等（fake timers）、**可见性补拉（切回立即拉一次 / visibilitychange 与 focus 只补一次 / 页面仍不可见不补 / stopLive 后不再响应）**。
