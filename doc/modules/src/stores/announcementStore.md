# announcementStore（公告列表与已读状态）

> 源文件：`src/stores/announcementStore.ts`

## 职责

比赛公告列表 + 客户端已读状态（HOJ 无已读概念，Rust 端按「比赛 + 用户」本地持久化）。为 ActivityBar 红点提供 `unreadCount`，为公告页提供列表与 `markAllRead`。

## 核心类型/函数

模块级 `poller: Poller | null` — 轮询器句柄放模块作用域（不进响应式系统），由 `stopLive` 回收；登出经 `stores/session` 统一停止。

| 名称 | 签名 | 用途 |
|------|------|------|
| state | `announcements` / `readIds` / `total` / `isLoading` / `isLive` / `error` / `contestId` | 列表（服务端顺序原样保留）、已读 ID 数组、请求状态、轮询上下文 |
| `unreadCount` | getter `=> number` | 未读数（ActivityBar 红点数据源） |
| `isUnread` | getter `(id) => boolean` | 单条未读判定（列表圆点） |
| `hasData` | getter `=> boolean` | 区分「空列表」与「尚未请求」 |
| `load` | `(contestId) => Promise<void>` | 并行拉列表 + 已读集合；**已读拉取失败降级为空集**（宁可多显红点，不可漏报公告）；列表失败写 error 并抛出 |
| `refresh` | `() => Promise<void>` | 轮询用刷新：吞掉异常，不打断轮询节奏 |
| `markAllRead` | `() => Promise<void>` | 乐观更新本地 → 后端持久化 → 以后端合并结果为准；**持久化失败回滚本地**（红点复发优于假已读） |
| `startLive` | `(contestId, isPaused?) => void` | 立即拉取一次（红点不等轮询周期）+ 60s±10s createPoller（`document.hidden` 暂停）；由外壳 ContestLayout 启动 |
| `stopLive` | `() => void` | 停止轮询（离开工作台/比赛结束/登出） |

## 直接依赖

- `pinia`
- `@/types/announcement`（仅类型）
- `@/services/announcement.service`
- `@/utils/polling`（`createPoller`）
- `@/utils/error`（`errorMessage` —— 错误文案收敛）
- `@/utils/logger`（`createLogger` —— 作用域日志）

## 被依赖

- `views/ContestLayout.vue` — 外壳启动/停止轮询（红点在全部页面保持鲜活）
- `views/AnnouncementsView.vue` — 列表渲染 + 进入页面 `markAllRead`
- `components/layout/ActivityBar.vue` — `unreadCount` 红点徽标
- `stores/session.ts` — 登出清理：`stopLive()` + `$reset()`

## 逻辑流程

```
ContestLayout（比赛就绪）→ startLive(cid, 比赛已结束判据)
  ├─ refresh() 立即一次
  └─ createPoller(60s±10s, hidden 暂停) → refresh()

AnnouncementsView onMounted
  → contestStore.whenLoaded() → refresh() → markAllRead()
     ├─ 本地乐观：readIds += 未读 ids（红点立即消失）
     ├─ service.markRead → 后端合并集合 → 替换 readIds
     └─ 失败 → 回滚 readIds + log.error 记录（utils/logger 作用域日志）
```

设计要点：

- **未读语义（产品决策）**：红点 = `unreadCount > 0`；进入公告页即全部已读；已读按「比赛+用户」落盘，重启/重登不复发。
- 轮询节奏 60s±10s：公告由裁判组低频发布，无需榜单级实时性；抖动打散全场客户端相位。
- 已读集合用数组承载（保留响应式），判定走 getter 内部 `Set`。

## 测试

`src/stores/__tests__/announcementStore.spec.ts`：load 成功/已读降级/列表失败、unreadCount 与 isUnread、markAllRead 乐观更新+失败回滚+无未读短路+空 contestId 静默、refresh 吞错、startLive 立即拉取+周期刷新+stopLive+重复 startLive 幂等（fake timers）。
