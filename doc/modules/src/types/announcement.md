# announcement（公告类型）

> 源文件：`src/types/announcement.ts`

## 职责

比赛公告的跨端契约类型，对应 Rust `core::entity::announcement`。

## 核心类型/函数

| 名称 | 结构 | 说明 |
|------|------|------|
| `Announcement` | `{ id, title, content, author, createdAt, updatedAt }` | 公告实体；`content` 为 Markdown/HTML 混合（渲染必须经 `renderMarkdown` 出口消毒）；时间为 **epoch 秒**（与 `Contest.startTime` 同口径） |
| `AnnouncementPage` | `{ records, total, size, current, pages }` | 分页结果 |

## 直接依赖

无（纯类型模块）。

## 被依赖

- `bridge/announcement.bridge.ts`、`services/announcement.service.ts`、`stores/announcementStore.ts`、`views/AnnouncementsView.vue`

## 逻辑流程

Rust 实体 `#[serde(rename_all = "camelCase")]` 序列化后与本类型逐字段对应；HOJ 未返回的字段由 Rust 端宽松容错补默认值，前端不做二次兜底。
