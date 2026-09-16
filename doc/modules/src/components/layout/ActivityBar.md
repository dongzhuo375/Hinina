# ActivityBar（左侧活动栏）

> 源文件：`src/components/layout/ActivityBar.vue`

## 职责

工作台左侧图标导航栏：题目 / 榜单 / 评测 / 公告四个主入口 + 底部设置，按当前路由高亮并渲染激活指示条；公告入口挂**真实未读红点徽标**（数据源 `announcementStore.unreadCount`）。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `activeKey` | computed | 由 `route.path` 前缀推导当前高亮项；`/contest/problems` 与 `/contest/problem/:displayId` 均归属「题目」（解题页仍高亮题目入口） |
| `itemClass(key)` | `(key: string) => string` | 导航项样式：激活=浅紫底+紫字，悬停=中性浅底 |
| 公告红点 | 模板内条件渲染 | `unreadCount > 0 && activeKey !== 'announcements'` 时在公告图标角渲染 8px 玫红圆点（白色描边）；处于公告页时不显示（进入该页即全部已读，避免标记往返期间红点残留） |

## 直接依赖

- `vue` / `vue-router`（`useRoute` + `router-link`，导航目标：ProblemSet / Rank / Submissions / Announcements / Settings）
- `@/stores/announcementStore`（仅 `unreadCount` 只读消费）

## 被依赖

- `views/ContestLayout.vue` — 外壳左栏

## 逻辑流程

```
route.path 前缀匹配 → activeKey → 对应 router-link 高亮 + 左缘 3px 紫色指示条
announcementStore.unreadCount > 0 且不在公告页 → 公告图标角红点
  （轮询由外壳 ContestLayout 编排，红点在全部页面保持鲜活；进入公告页 markAllRead 后消失）
```

设计要点：

- 导航纯展示 + 路由跳转；唯一的 store 消费是公告未读数（只读 getter，不发起请求）。
- **红点不造假**：徽标严格来自服务端公告列表与本地已读集合的差集，接口未接入前不渲染（历史约定），现已接入真实数据。
- 图标内联 SVG（离线客户端不引外部字体/CDN）。
