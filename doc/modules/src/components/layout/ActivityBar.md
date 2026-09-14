# ActivityBar（左侧活动栏）

> 源文件：`src/components/layout/ActivityBar.vue`

## 职责

工作台左侧图标导航栏：题目 / 榜单 / 评测 / 公告四个主入口 + 底部设置，按当前路由高亮并渲染激活指示条。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `activeKey` | computed | 由 `route.path` 前缀推导当前高亮项；`/contest/problems` 与 `/contest/problem/:displayId` 均归属「题目」（解题页仍高亮题目入口） |
| `itemClass(key)` | `(key: string) => string` | 导航项样式：激活=浅紫底+紫字，悬停=中性浅底 |

## 直接依赖

- `vue` / `vue-router`（`useRoute` + `router-link`，导航目标：ProblemSet / Rank / Submissions / Announcements / Settings）

## 被依赖

- `views/ContestLayout.vue` — 外壳左栏

## 逻辑流程

```
route.path 前缀匹配 → activeKey → 对应 router-link 高亮 + 左缘 3px 紫色指示条
```

设计要点：

- 纯展示 + 路由跳转，不触任何 store/service/bridge。
- **公告不画假红点徽标**：待公告接口接入后再显示（与 PlaceholderView「不渲染假数据」口径一致）。
- 图标内联 SVG（离线客户端不引外部字体/CDN）。
