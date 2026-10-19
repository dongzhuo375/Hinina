# LoadingSpinner（通用加载动画）

> 源文件：`src/components/common/LoadingSpinner.vue`

## 职责

全局统一的加载态组件：旋转圆环 + 可选文案，供各视图/组件在异步数据未到达时复用。

## 核心类型/函数

| 名称 | 类型 | 用途 |
|------|------|------|
| `message` | prop `string?` | 加载文案（如「正在加载榜单…」）；缺省只渲染圆环 |

## 直接依赖

无（纯模板组件，样式取 global.css 的 `--color-primary` 等变量）

## 被依赖

- `views/ProblemSetView.vue` / `views/ProblemSolveView.vue`（题面加载）、`views/RankView.vue`（isBootstrapping 满屏加载）
- `views/SubmissionsView.vue` / `views/SubmissionDetailView.vue` / `views/AnnouncementsView.vue` / `views/SettingsView.vue`（首屏加载态）

## 逻辑流程

```
<div role="status">（gap-4 / py-12）
  旋转圈：h-6 w-6 animate-spin rounded-full border border-[--border-color] border-t-[--color-primary]
  message 有值 → 文案；无值 → 仅圆环（CSS animate-spin，无 JS 定时器）
```

设计要点：

- 纯 CSS 动画、无状态、无副作用——加载组件自身绝不引入异步或定时器。
- **`role="status"`**（PR #36）：加载态对读屏可见，页面不会在「看起来没反应」中空转。
- 视觉收敛为细线圆环（中性底 + 主色顶边），与赛场「克制」的版式语言一致；
  原 8px 粗边纯主色圆环在浅色页面上过于抢眼。
