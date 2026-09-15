# PlaceholderView（功能开发中占位页）

> 源文件：`src/views/PlaceholderView.vue`

## 职责

评测 / 公告 / 设置三个未接入服务端接口的工作台页面共用的「功能开发中」占位视图，文案由路由 `props` 注入，避免为每个空页面复制一份模板。

## 核心类型/函数

| 名称 | 类型 | 用途 |
|------|------|------|
| `title` | prop `string` | 页面标题（如「评测」） |
| `description` | prop `string` | 功能说明文案（描述该模块接入后的能力） |

## 直接依赖

无（纯模板组件，图标为内联 SVG）

## 被依赖

- `router/index.ts` — 三条路由复用同一组件、以静态 `props` 区分文案：
  `Submissions`（/contest/submissions）、`Announcements`（/contest/announcements）、`Settings`（/contest/settings）

## 逻辑流程

```
路由 props { title, description } → 居中卡片渲染（内联 SVG 图标 + 标题 + 描述）
底部固定提示「该模块尚未接入服务端接口」
```

设计要点：

- 明确告知「未接入」而不是渲染假数据：评测记录数量、公告红点徽标等一律不画，
  与 ActivityBar 中「公告红点待接口接入后再显示」的口径一致。
- 图标内联 SVG——离线客户端不引入外部图标字体/CDN（全项目统一约定）。
