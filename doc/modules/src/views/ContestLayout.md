# ContestLayout（比赛工作台外壳）

> 源文件：`src/views/ContestLayout.vue`

## 职责

比赛工作台的布局外壳：TopBar（顶栏）+ ActivityBar（左侧活动栏）+ `<router-view>`（子视图）+ StatusBar（底栏），并在挂载时拉取一次比赛数据供所有子视图共享。

## 核心类型/函数

`<script setup>`，无 props/emits：

| 名称 | 用途 |
|------|------|
| `onMounted` 钩子 | `contest` 为空且不在加载中时调用 `contestStore.loadContest()`，失败静默（原因已由 store 写入 `error`，StatusBar 与各视图负责兜底展示） |

## 直接依赖

- `vue`（`onMounted`）
- `@/components/layout/TopBar.vue` / `ActivityBar.vue` / `StatusBar.vue`
- `@/stores/contestStore`

## 被依赖

- `router/index.ts` — 路由 `Contest`（`/contest`，`meta.requiresAuth: true`，redirect 到 ProblemSet），子路由：ProblemSet / ProblemSolve / Rank / Submissions / Announcements / Settings

## 逻辑流程

```
路由守卫（meta.requiresAuth）保证会话有效 → 外壳挂载
  → contest 未加载且无在途请求 → contestStore.loadContest()（并发去重由 store 保证）
  → TopBar / StatusBar / 子视图共享同一份 contestStore 状态
布局：flex 纵向（TopBar / [ActivityBar + main(router-view)] / StatusBar），
     main 用 min-w-0 + overflow-hidden 约束，横向滚动留给子视图内部
```

设计要点：

- **会话有效性不在本组件校验**：由路由守卫（`meta.requiresAuth` + `authStore`）统一保证，
  外壳只负责数据与布局。
- 外壳拉取与子视图兜底拉取（RankView / ProblemSetView / ProblemSolveView 各自 `ensureContest`）
  可能同时发生，靠 `contestStore.loadContest` 的模块级 in-flight Promise 去重收敛为一次 IPC。
- 设置页也放在外壳内（子路由），切换时不丢失顶栏/活动栏/状态条。
- 分层约束：View 只 import store 与组件，不触 `@/bridge` / `@/services`（本组件仅 store）。
