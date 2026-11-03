# announcement.spec.ts（E2E：赛事公告）

> 源文件：`e2e/specs/announcement.spec.ts`；2026-11 审计 C4 补缺。

## 职责

公告列表渲染、进入即标已读、新公告事件驱动的红点与未读清除。

## 核心类型/函数

三个用例：

1. **列表渲染 + 进入即标已读**：公告标题/正文可见、`get_read_announcement_ids`
   与 `mark_announcements_read` 各 ≥1 次、ActivityBar 红点不亮。
2. **新公告事件旅程**：先进入公告页清空存量未读 → 回题目页 → 页内
   `pushAnnouncement()` + `emitEvent('announcements-published', { contestId, newIds })`
   → 事件即刷新（不等 60s 轮询）、红点亮起（此刻「不在看」不自动标已读）→
   再进公告页：共 2 条、mark 载荷恰含新公告 id（fake 已读集合**有状态**，
   未读集 = 列表 − 已读）、读完红点清除。

## 直接依赖

- `../fake-tauri`（`callCount` / `loginAndEnterContest` / 控制面 `pushAnnouncement` / `emitEvent`）
- `src/views/AnnouncementsView.vue`、`src/components/layout/ActivityBar.vue`（红点）

## 被依赖

- 无（叶子 spec）

## 逻辑流程

3. **跨 reload 持久化**：进入即标已读 → 题目页 pushAnnouncement + 事件 → reload
   → 公告列表仍 2 条（追加存活）、reload 后 mark 载荷恰为 `['702']`（已读集不回退；
   `calls` 为文档级、reload 清零，载荷即 reload 后的全部 mark）。

## 逻辑流程（更新）

`main.ts` 的公告事件订阅 → `announcementStore.refresh()`（事件只是刷新触发）；
未读计数 = 列表 − 已读集合；进入公告页（isWatching）→ load 落地即 markAllRead。
fake 的公告与已读集合在 sessionStorage 的 PersistedBackend 中跨 reload 存活。
