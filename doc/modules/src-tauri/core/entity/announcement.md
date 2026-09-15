# announcement

## 职责
定义比赛公告领域实体 `Announcement` 与分页结果 `AnnouncementPage`，供公告列表 Command / Service / Adapter 与前端共用。全部 camelCase 序列化，时间为 UTC 秒级时间戳（各 Adapter 负责从 OJ 原始格式统一转换）。

## 核心类型
- **`Announcement`** — 比赛公告 struct：
  - `id: String` — 公告 ID（HOJ 的数字 id 由 Adapter 转为字符串，与项目内「ID 统一用字符串传递」约定一致）
  - `title: String` — 标题
  - `content: String` — 公告正文（HTML/Markdown）；HOJ 可能返回 null，Adapter 映射时回退空串
  - `author: String` — 发布者用户名（来自 HOJ 的 `username` 字段）
  - `created_at: i64` / `updated_at: i64` — 创建/更新时间（UTC 秒级时间戳）
- **`AnnouncementPage`** — 分页结果（对应 MyBatis-Plus `IPage` 形状）：`records: Vec<Announcement>`, `total`, `size`, `current`, `pages`（均 `i64`）

## 直接依赖
- `serde::{Deserialize, Serialize}`

## 被依赖
- `core::provider::contest`（`ContestProvider::list_announcements` 返回 `AnnouncementPage`）
- `adapter::hoj`（`into_announcement` 把 `AnnouncementVO` 映射为 `Announcement`）
- `service::contest`（`list_announcements` 透传）
- `commands::contest_cmd`（`list_contest_announcements` Command 返回值）

## 逻辑流程
无（纯类型定义）。数据流：

```
HOJ AnnouncementVO（camelCase，含显式 null）
  → adapter::hoj::into_announcement（id 转字符串、username→author、时间转秒级时间戳、null content 回退空串）
  → AnnouncementPage（camelCase JSON 过 IPC）
  → 前端公告面板渲染
```

公告**不做缓存**（可能含裁判组临场发布的规则变更），已读状态为客户端本地特性（见 `service/contest/mod.md`）。
