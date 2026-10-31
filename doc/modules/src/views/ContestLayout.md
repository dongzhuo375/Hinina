# ContestLayout（比赛工作台外壳）

> 源文件：`src/views/ContestLayout.vue`

## 职责

比赛工作台的布局外壳：`TopBar`（顶栏，内嵌横向导航）+ `<router-view>` 子视图 + `StatusBar`（底栏）；挂载时拉取比赛数据、**按权威赛程刷新跨场缓存**、在比赛进行中启动公告轮询，并以每秒时钟驱动**赛程闸门**——赛程被移到未来时立即退回登录页，比赛结束时叠加结束覆盖层并在 2 秒后自动返回登录页。

**无会话最小壳**：`--settings`（管理方钥匙）会在无会话时经守卫直落设置路由（守卫条件豁免其鉴权，见 `router/index.md`），此时外壳退化为最小壳——自带 `data-tauri-drag-region` 拖动区标题栏（「Hinina 设置」+「返回登录」，`decorations: false` 下窗口移动全靠它）+ 内容区；**不渲染** TopBar/StatusBar（其数据全部依赖会话），`bootstrap` 直接跳过（无会话时认证拉取只会失败），不产生任何面向 OJ 的请求。判据是**四条件与**：钥匙启动 + 无会话 + 设置路由（`launchFlags.settings && sessionResolved && !isLoggedIn && route === 'Settings'`）——绑路由防「正常登出/会话失效（`user` 置空而 `sessionResolved` 仍为 true）在 `replace(Login)` 异步守卫完成前闪最小壳、子视图重挂」（评审二），绑钥匙防「普通启动在设置页上登出时闪最小壳」（评审三）：普通启动**永远**不进最小壳。

## 核心类型/函数

`<script setup>`，无 props/emits：

| 名称 | 签名 | 用途 |
|------|------|------|
| `nowMs` / `now` | `useNowMs()` / computed（秒） | 共享毫秒墙钟派生秒，驱动 `phase` 跃迁；scope dispose 清理 interval |
| `phase` | computed | `getContestPhase(getAuthoritativeSchedule(contestStore.contest, contestStore.brief), now)` |
| `bareShell` | computed | `systemService.launchFlags.value.settings && !auth.isLoggedIn && route.name === 'Settings'` —— 最小壳判据（钥匙 + 会话缺失 + 设置路由，三条件与）；**刻意不含 `sessionResolved`**：钥匙 + 存量过期会话启动时首个认证请求 401 会把它复位为 false，此刻正是最小壳的形态（评审四 P2-1） |
| `watch([phase, bareShell], …)` | — | **赛程闸门**（唯一退出决策点）：`upcoming` → 立即 `replace(Login)`；`ended` → `stopLive()` 后 `setTimeout(2_000)` 再 `replace(Login)`；每次先清 `exitTimer`。**最小壳（bareShell）时整体短路**——登出清理保留匿名 brief，phase 可非 none（评审四 P2-2），跳转会把维护者从设置页踢走；bareShell 翻转也重评并撤销在途退出计时器 |
| `bootstrap` | `() => Promise<void>` | 首行 `bareShell` 直接 return（跳过认证拉取与公告轮询）；否则 `whenLoaded()` → 跨场刷新 → 启动公告轮询，见逻辑流程 |
| `backToLogin` | `() => void` | 最小壳的「返回登录」按钮 → `replace(Login)`（正常登录页，含匿名简报与重试 UI） |
| `exitTimer` | 定时器句柄 | 结束退出延迟；模块级普通变量，不进响应式系统（墙钟 interval 已归 `useNowMs`） |
| `disposed` | `boolean` | 卸载后阻断在途异步续跑 |

## 直接依赖

- `vue`（`computed` / `onMounted` / `onUnmounted` / `ref` / `watch`）/ `vue-router`（`useRouter` / `useRoute`）
- `@/components/layout/TopBar.vue` / `StatusBar.vue`
- `@/stores/authStore`（`sessionResolved` / `isLoggedIn` —— 最小壳判据之一）
- `@/stores/contestStore` / `@/stores/announcementStore`
- `@/services/system.service`（`launchFlags` 同步快照 —— 最小壳判据之一；View → Service 直连属「路由级瞬态数据」许可）
- `@/utils/contest`（`getAuthoritativeSchedule` / `getContestPhase`）
- `@/composables/useNowMs`（毫秒墙钟，派生秒）

注：**不再 import `ActivityBar`** —— 导航条已移入 `TopBar` 第二行（PR #36）。

## 被依赖

- `router/index.ts` — 路由 `Contest`（`/contest`，`meta.requiresAuth: true`，redirect 到 ProblemSet），子路由：ProblemSet / ProblemSolve / Rank / Submissions / SubmissionDetail / Announcements / Settings

## 逻辑流程

```
onMounted → bootstrap()（墙钟 interval 由 useNowMs 自持）
bootstrap:
  await contestStore.whenLoaded()             // 复用在途请求（P59），子视图先触发也走同一链路
  ├ disposed → return
  ├ contest.id 与 brief.id 都存在且不同
  │   → await contestStore.loadContest()      // 简报已切到新比赛，刷新认证数据
  └ 失败 → return（原因已由 store 写入 error，StatusBar 与各视图兜底）
  contestId 存在 且 phase === 'running' → announcementStore.startLive(contestId, () => contest.status === 1)

watch(phase, immediate):
  upcoming → router.replace(Login)                  // 赛程被移到未来：立即回入口等待
  ended    → announcementStore.stopLive()
             2s 后 router.replace(Login)            // 保留会话与工作区，让离页保存链路落盘
  两次分支前都先清 exitTimer（避免 upcoming 之后又被旧的 ended 定时器踢走）

onUnmounted → disposed = true；清退出定时器；announcementStore.stopLive()（墙钟由 scope dispose 清理）

模板：
  根 .contest-theme（启用赛场 CSS 令牌）
  bareShell（最小壳分支：无会话 + 设置路由）：
    header[data-tauri-drag-region]（「Hinina 设置」+「返回登录」按钮）
    <main><router-view /></main>
  否则（正常外壳）：
  <TopBar :navigation-disabled="phase === 'ended' || phase === 'upcoming'" />
  中间容器同条件 :inert（禁点禁焦点，覆盖子视图）
  <StatusBar />
  phase === 'ended' 时叠加覆盖层（absolute inset-x-0 bottom-0 top-10 z-50，半透明 + 背景模糊）
    内含 role="alert" 卡片：CONTEST CLOSED / 「比赛已结束」/「感谢参赛，即将返回登录入口。」
```

设计要点：

- **会话有效性不在本组件校验**：由路由守卫（`meta.requiresAuth` + 赛程闸门）统一保证，
  外壳只负责数据、布局与赛程退出。唯一例外是最小壳：守卫对「钥匙启动 + 设置路由」
  条件豁免鉴权，外壳以 `bareShell` 识别该形态并退化为最小壳，**不发起任何认证/OJ
  请求**。
- **最小壳必须自带拖动区**：App 的全局标题栏在 matched 含 Contest 时不渲染，
  窗口 `decorations: false`，无拖动区则窗口无法移动。
- **最小壳刻意不含最小化/关闭按钮（评审 M1 的裁定）**：窗口控制在 TopBar，最小壳
  不渲染 TopBar，设置页上只剩 Alt+F4 / 任务栏；点「返回登录」到登录页后 App 兜底
  标题栏（含完整窗口控制）即恢复。维护场景临时且退出路径存在，不为一次性形态
  抽共享 WindowControls 组件——若未来最小壳使用频率上升再议。
- **最小壳判据必须含路由条件（评审二抓出）**：`logout()` 成功后 `sessionResolved`
  保持 true、`user` 置空是同步的，重渲染先于 `replace(Login)` 的异步守卫完成——
  只判会话会让比赛页外壳在登出瞬间闪成最小壳并重挂子视图。绑定 Settings 路由
  后，比赛子路由的闪帧在结构上不可能。
- **还必须含钥匙条件（评审三）**：已登录维护者经隐藏入口在设置页上登出时，路由
  同样是 Settings——绑钥匙（`launchFlags` 同步快照）让**普通启动永远不进最小壳**。
  快照由守卫首航填充（早于任何组件挂载），同步读取不会把闪帧反转到钥匙启动的
  首帧。
- **跨场缓存刷新（PR #35 收尾）**：登录页的匿名简报可能已经切到下一场比赛，
  而 `contestStore.contest` 仍是上一场的缓存；此时 `whenLoaded()` 因 contest 非空直接返回，
  必须比对 `contest.id` 与 `brief.id` 并主动 `loadContest()` —— 否则题目、榜单、提交、
  公告会全部继续绑定旧比赛（现象是「进了新赛场但题目是上一场的」）。
- **`phase` 用权威赛程**：`getAuthoritativeSchedule(contest, brief)` 保证「旧认证数据 +
  新简报」这种跨场组合下判据正确（见 `utils/contest.md`）。
- **退出必须延迟 2 秒**：比赛结束瞬间若立刻 `replace`，正在进行的代码落盘/离页保存会被
  打断；延迟窗口内先 `stopLive()` 停掉公告轮询，会话与工作区保持不动，由既有离页保存
  链路完成落盘。这个延迟也是覆盖层存在的意义 —— 让选手看到「比赛已结束」而不是突兀跳走。
- **`upcoming` 立即退出**：赛程被改到未来（组织者顺延）时留在工作台只会让选手看到
  一堆过期数据，直接回入口页等待，到点由 LoginView 自动进场。
- **`inert` + `navigationDisabled` 双保险**：结束/未开始时子视图与导航同时不可交互，
  防止在覆盖层下继续操作（提交、切题）造成「已结束还在提交」的错觉。
- **公告轮询归外壳而非公告页**：未读红点徽标在所有页面可见，轮询必须与工作台同生命
  周期；公告页自身只声明 `isWatching`（「用户正在看」），标记已读由 `store.load` 在
  该标记为真且页面可见时统一完成（详见 `stores/announcementStore.md`）。
- 设置页也放在外壳内（子路由），切换时不丢失顶栏/状态条。
- 分层约束：View 只 import store 与组件，不触 `@/bridge`；`system.service` 的
  `launchFlags` 快照是唯一的 View → Service 直连（属「路由级瞬态数据」许可，
  最小壳判据需要同步值）。

## 测试

`src/views/__tests__/ContestLayout.spec.ts`（jsdom，mock `useRouter` / `announcementStore` / `contestService.loadConfiguredContest`，stub 三个 layout 子组件，赛程 `startTime=900 / endTime=1002`）锁定：到点后出现 `role="alert"` 的结束卡片（父容器含 `top-10`）、存在 `[inert]`、`stopLive` 恰好一次、`replace` 未立即调用，2 秒后 `replace({ name: 'Login' })` 恰好一次；服务端 `status=1` 提前结束时同样处理；赛程被移到未来时立即 `replace(Login)` 且 `startLive` 未被调用；「旧 id 已结束 contest + 新 brief」以简报为准、不出覆盖层；brief 指向新比赛时 `loadConfiguredContest` 被调用一次且 `startLive` 用新 contestId；卸载后推进 5 秒 `replace` 未被调用且无残留定时器。最小壳用例（`sessionResolved=true` 且 `user=null`）：渲染「Hinina 设置」拖动区标题与「返回登录」、不出 TopBar/StatusBar、`loadConfiguredContest` 与 `startLive` 均未被调用、点击返回登录 `replace(Login)`。
