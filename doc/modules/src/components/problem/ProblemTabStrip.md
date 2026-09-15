# ProblemTabStrip（题目快速切换条）

> 源文件：`src/components/problem/ProblemTabStrip.vue`

## 职责

解题页顶部的横向 chips（A/B/C…）快速切题条：当前题高亮 + 下划线指示，每个 chip 带我的提交状态点。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `problems` | computed | `contestStore.problems` |
| `currentDisplayId` | computed | `route.params.displayId`（字符串化） |
| `statusDotClass(status)` | `(number) => string` | 状态点颜色：1=已AC（绿）/ 2=尝试过（amber）/ 0=未提交（**不显示点**） |
| `go(displayId)` | `(string) => void` | 点击跳转 `ProblemSolve` 路由；当前题点击无效（不重复导航） |

## 直接依赖

- `vue` / `vue-router`（`useRoute` + `router.push`）
- `@/stores/contestStore`（题目列表）、`@/stores/problemStore`（`statusOf(problemId)` 状态点）

## 被依赖

- `views/ProblemSolveView.vue` — 解题页顶栏

## 逻辑流程

```
contestStore.problems → chips 渲染（title 悬浮显示 displayId + displayTitle）
problemStore.myStatus 更新 → 状态点响应式跃迁
点击 chip → router.push(ProblemSolve, { displayId }) → ProblemSolveView 的
            watch(displayId) 触发切题加载链（落盘旧工作区 → 加载新题）
题目列表为空 → 显示「题目列表未加载」占位文本
```

设计要点：

- 数据直接取自 contestStore / problemStore（View → Store 分层，组件不触 Service/Bridge）。
- 横向滚动但隐藏滚动条（`scrollbar-width: none` + `::-webkit-scrollbar`），贴合设计稿的 tab 条样式。
- 状态点用 `problemId`（pid）查询 `statusOf`，与卡片一致；chip 本身用 `displayId` 导航，
  两套 ID 的分工见 ProblemSolveView 文档。
