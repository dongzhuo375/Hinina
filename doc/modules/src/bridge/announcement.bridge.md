# announcement.bridge（公告 IPC 封装）

> 源文件：`src/bridge/announcement.bridge.ts`

## 职责

比赛公告列表与本地已读状态的 Tauri IPC 薄封装。已读状态是客户端特性（HOJ 无已读概念），由 Rust 端按「比赛 + 用户」持久化到本地文件。

## 核心类型/函数

| 名称 | 签名 | 对应命令 |
|------|------|----------|
| `listContestAnnouncements` | `(contestId, currentPage, limit) => Promise<AnnouncementPage>` | `list_contest_announcements` |
| `getReadAnnouncementIds` | `(contestId) => Promise<string[]>` | `get_read_announcement_ids`（uid 取自后端会话；未登录返回空） |
| `markAnnouncementsRead` | `(contestId, ids) => Promise<void>` | `mark_announcements_read`（后端合并去重持久化） |

## 直接依赖

- `@/bridge`（`ipcInvoke` 统一错误归一化）
- `@/types/announcement`（仅类型）

## 被依赖

- `services/announcement.service.ts`（唯一消费方）

## 逻辑流程

纯透传：参数经 Tauri invoke 传给同名 Rust 命令，错误由 `bridge/index.ts` 归一化为 `IpcError`（Auth 变体触发全局会话守卫）。
