# announcement.service（公告服务）

> 源文件：`src/services/announcement.service.ts`

## 职责

比赛公告获取与本地已读状态编排：透传 Bridge 调用、把已读 ID 数组组装为 `Set`、提供未读过滤纯函数。不做缓存（公告必须实时）。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `AnnouncementService.listAnnouncements` | `(contestId) => Promise<AnnouncementPage>` | 单页大容量拉取（`ANNOUNCEMENT_PAGE_SIZE = 100`，比赛公告总量通常 < 30 条，免去翻页交互） |
| `AnnouncementService.getReadIds` | `(contestId) => Promise<Set<string>>` | 当前用户在该比赛下的已读 ID 集合 |
| `AnnouncementService.markRead` | `(contestId, ids) => Promise<Set<string>>` | 标记已读并返回**后端合并后的权威集合**（前端直接替换本地状态）；空 ids 短路为只读查询 |
| `AnnouncementService.filterUnread` | `(announcements, readIds) => Announcement[]` | 未读过滤（保持列表顺序） |
| `announcementService` | 单例 | 全局唯一实例 |

## 直接依赖

- `@/bridge/announcement.bridge`
- `@/types/announcement`（仅类型）

## 被依赖

- `stores/announcementStore.ts`（唯一消费方）

## 逻辑流程

```
listAnnouncements → bridge.listContestAnnouncements(cid, 1, 100)
markRead(ids)     → ids 为空 ? getReadIds : bridge.markAnnouncementsRead → getReadIds（合并后真值）
```

设计要点：已读集合的权威源在 Rust 端文件（合并去重持久化），前端每次标记后重新读取合并结果，避免两端集合漂移。
