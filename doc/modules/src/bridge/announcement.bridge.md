# announcement.bridge（公告 IPC 封装）

> 源文件：`src/bridge/announcement.bridge.ts`

## 职责

比赛公告列表、本地已读状态与**新公告事件**的 Tauri IPC 薄封装。已读状态是客户端特性（HOJ 无已读概念），由 Rust 端按「比赛 + 用户」持久化到本地文件；新公告事件由 Rust 侧比对公告基线后发布（`CoreEvent::AnnouncementChanged`），经 `src-tauri/src/main.rs` 的前端事件桥转发到 `announcements-published` 通道。

## 核心类型/函数

| 名称 | 签名 | 对应命令 |
|------|------|----------|
| `listContestAnnouncements` | `(contestId, currentPage, limit) => Promise<AnnouncementPage>` | `list_contest_announcements` |
| `getReadAnnouncementIds` | `(contestId) => Promise<string[]>` | `get_read_announcement_ids`（uid 取自后端会话；未登录返回空） |
| `markAnnouncementsRead` | `(contestId, ids) => Promise<void>` | `mark_announcements_read`（后端合并去重持久化） |
| `ANNOUNCEMENTS_PUBLISHED_EVENT` | 常量 `'announcements-published'` | 前端事件通道名，必须与 `src-tauri/src/main.rs` 的 emit 字面量一致 |
| `AnnouncementsPublishedPayload` | `{ contestId: string; newIds: string[] }` | 事件载荷；`newIds` = 本次新出现的公告 ID |
| `onAnnouncementsPublished` | `(handler) => Promise<UnlistenFn>` | 包一层 Tauri `listen`：**校验载荷形状**（`payload` 存在且 `contestId` 为 string，否则直接丢弃 —— 事件来自 Rust 且通道公开，越界数据不该进 store），`newIds` 缺省补 `[]`。返回取消订阅函数，调用方（组合根）应保留以便注销 |

## 直接依赖

- `@/bridge`（`ipcInvoke` 统一错误归一化）
- `@tauri-apps/api/event`（`listen` + `UnlistenFn` 类型 —— 与 `workspace.bridge` 同款的事件订阅原语）
- `@/types/announcement`（仅类型）

## 被依赖

- `services/announcement.service.ts`（三个命令的消费方）
- `main.ts` — `installAnnouncementListener` 订阅新公告事件（事件类 API 不经 service：没有业务编排可包，组合根直接消费 bridge）

## 逻辑流程

```
命令类（纯透传）：
  announcement.service.*
    → ipcInvoke('list_contest_announcements' | 'get_read_announcement_ids'
                | 'mark_announcements_read', …)
    → Rust commands::contest_cmd → ContestService（本地已读存储 / ContestProvider）
  错误由 `bridge/index.ts` 归一化为 `IpcError`（Auth 变体触发全局会话守卫）

事件类（**由前端拉取触发，Rust 侧不自行轮询**）：
  list_contest_announcements → ContestService 与上次基线比对 → 出现新 ID
    → 发布 CoreEvent::AnnouncementChanged { contest_id, new_ids }
    → src-tauri/src/main.rs 的前端事件桥 emit('announcements-published', { contestId, newIds })
    → onAnnouncementsPublished：载荷校验 → handler({ contestId, newIds: newIds ?? [] })
    → main.ts 判 contestId 是否当前比赛 → announcementStore.refresh()
```

设计要点：

- **事件只报「有新公告」，不夹带公告内容**：前端仍需走 `listContestAnnouncements` 取正文（列表/分页/正文渲染的唯一来源），事件只负责把「状态已变」提前告知 —— 避免事件载荷与服务端列表两份数据口径漂移。
- **订阅方不必自己去重**：Rust 侧只在确认出现**新 ID** 时下发，首次拉取不发（否则每次启动都会假报一条新公告）。
