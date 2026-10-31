# ContestLayout（比赛工作台外壳）

> 源文件：`src/views/ContestLayout.vue`

## 职责

比赛工作台的布局外壳：`TopBar`（顶栏，内嵌横向导航）+ `<router-view>` 子视图 + `StatusBar`（底栏）；挂载时拉取比赛数据、**按权威赛程刷新跨场缓存**、在比赛进行中启动公告轮询，并以每秒时钟驱动**赛程闸门**——赛程被移到未来时立即退回登录页，比赛结束时叠加结束覆盖层并在 2 秒后自动返回登录页。

**无会话最小壳**：`--settings`（管理方钥匙）会在无会话时经守卫直落设置路由（守卫豁免其 `requiresAuth`），此时外壳退化为最小壳——自带 `data-tauri-drag-region` 拖动区标题栏（「Hinina 设置」+「返回登录」，`decorations: false` 下窗口移动全靠它）+ 内容区；**不渲染** TopBar/StatusBar（其数据全部依赖会话），`bootstrap` 直接跳过（无会话时认证拉取只会失败），不产生任何面向 OJ 的请求。

## 核心类型/函数

`<script setup>`，无 props/emits：

| 名称 | 签名 | 用途 |
|------|------|------|
| `nowMs` / `now` | `useNowMs()` / computed（秒） | 共享毫秒墙钟派生秒，驱动 `phase` 跃迁；scope dispose 清理 interval |
| `phase` | computed | `getContestPhase(getAuthoritativeSchedule(contestStore.contest, contestStore.brief), now)` |
| `sessionless` | computed | `auth.sessionResolved && !auth.isLoggedIn` —— 最小壳判据；守卫在导航前已完成 `checkSession`，进入本组件时该值已定 |
| `watch(phase, …, { immediate: true })` | — | **赛程闸门**（唯一退出决策点）：`upcoming` → 立即 `replace(Login)`；`ended` → `stopLive()` 后 `setTimeout(2_000)` 再 `replace(Login)`；每次先清 `exitTimer`。无会话时 phase 恒为 `none`，两个分支都不触发 |
| `bootstrap` | `() => Promise<void>` | 首行 `sessionless` 直接 return（跳过认证拉取与公告轮询）；否则 `whenLoaded()` → 跨场刷新 → 启动公告轮询，见逻辑流程 |
| `backToLogin` | `() => void` | 最小壳的「返回登录」按钮 → `replace(Login)`（正常登录页，含匿名简报与重试 UI） |
| `exitTimer` | 定时器句柄 | 结束退出延迟；模块级普通变量，不进响应式系统（墙钟 interval 已归 `useNowMs`） |
| `disposed` | `boolean` | 卸载后阻断在途异步续跑 |

## 直接依赖

- `vue`（`computed` / `onMounted` / `onUnmounted` / `ref` / `watch`）/ `vue-router`（`useRouter`）
- `@/components/layout/TopBar.vue` / `StatusBar.vue`
- `@/stores/authStore`（`sessionResolved` / `isLoggedIn` —— 最小壳判据）
- `@/stores/contestStore` / `@/stores/announcementStore`
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
  sessionless（最小壳分支）：
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
  外壳只负责数据、布局与赛程退出。唯一例外是最小壳：守卫对设置路由豁免鉴权，
  外壳以 `sessionless` 识别该形态并退化为最小壳，**不发起任何认证/OJ 请求**。
- **最小壳必须自带拖动区**：App 的全局标题栏在 matched 含 Contest 时不渲染，
  窗口 `decorations: false`，无拖动区则窗口无法移动。
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
- 分层约束：View 只 import store 与组件，不触 `@/bridge` / `@/services`（本组件仅 store）。

## 测试

`src/views/__tests__/ContestLayout.spec.ts`（jsdom，mock `useRouter` / `announcementStore` / `contestService.loadConfiguredContest`，stub 三个 layout 子组件，赛程 `startTime=900 / endTime=1002`）锁定：到点后出现 `role="alert"` 的结束卡片（父容器含 `top-10`）、存在 `[inert]`、`stopLive` 恰好一次、`replace` 未立即调用，2 秒后 `replace({ name: 'Login' })` 恰好一次；服务端 `status=1` 提前结束时同样处理；赛程被移到未来时立即 `replace(Login)` 且 `startLive` 未被调用；「旧 id 已结束 contest + 新 brief」以简报为准、不出覆盖层；brief 指向新比赛时 `loadConfiguredContest` 被调用一次且 `startLive` 用新 contestId；卸载后推进 5 秒 `replace` 未被调用且无残留定时器。最小壳用例（`sessionResolved=true` 且 `user=null`）：渲染「Hinina 设置」拖动区标题与「返回登录」、不出 TopBar/StatusBar、`loadConfiguredContest` 与 `startLive` 均未被调用、点击返回登录 `replace(Login)`。
