# ContestLayout（比赛工作台外壳）

> 源文件：`src/views/ContestLayout.vue`

## 职责

比赛工作台的布局外壳：TopBar（顶栏）+ ActivityBar（左侧活动栏）+ `<router-view>`（子视图）+ StatusBar（底栏）；挂载时经 `whenLoaded()` 拉取比赛数据供所有子视图共享，并在比赛就绪后启动公告轮询（ActivityBar 未读红点在全部页面保持鲜活）。

## 核心类型/函数

`<script setup>`，无 props/emits：

| 名称 | 用途 |
|------|------|
| `onMounted` 钩子 | `contestStore.whenLoaded()`（失败静默，原因已由 store 写入 `error`）→ 拿到 contestId 后 `announcementStore.startLive(contestId, () => contest?.status === 1)`（比赛结束即停） |
| `onUnmounted` 钩子 | `announcementStore.stopLive()`（离开工作台回收公告轮询定时器） |

## 直接依赖

- `vue`（`onMounted` / `onUnmounted`）
- `@/components/layout/TopBar.vue` / `ActivityBar.vue` / `StatusBar.vue`
- `@/stores/contestStore` / `@/stores/announcementStore`

## 被依赖

- `router/index.ts` — 路由 `Contest`（`/contest`，`meta.requiresAuth: true`，redirect 到 ProblemSet），子路由：ProblemSet / ProblemSolve / Rank / Submissions / SubmissionDetail / Announcements / Settings

## 逻辑流程

```
路由守卫（meta.requiresAuth）保证会话有效 → 外壳挂载
  → contestStore.whenLoaded()（并发去重由 store 保证）
  → contestId 就绪 → announcementStore.startLive(contestId, 比赛已结束判据)
  → TopBar / StatusBar / 子视图共享同一份 contestStore 状态
onUnmounted → announcementStore.stopLive()
布局：flex 纵向（TopBar / [ActivityBar + main(router-view)] / StatusBar），
     main 用 min-w-0 + overflow-hidden 约束，横向滚动留给子视图内部
```

设计要点：

- **会话有效性不在本组件校验**：由路由守卫（`meta.requiresAuth` + `authStore`）统一保证，
  外壳只负责数据与布局。
- 外壳拉取与子视图兜底拉取（各视图 `ensureContest`）可能同时发生，统一经
  `contestStore.whenLoaded()` 的模块级 in-flight Promise 去重收敛为一次 IPC（P59）。
- **公告轮询归外壳而非公告页**：未读红点徽标在所有页面可见，轮询必须与工作台同生命
  周期；公告页自身只负责进入时 `markAllRead`。
- 设置页也放在外壳内（子路由），切换时不丢失顶栏/活动栏/状态条。
- 分层约束：View 只 import store 与组件，不触 `@/bridge` / `@/services`（本组件仅 store）。
