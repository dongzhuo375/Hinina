# index（路由表与会话/赛程守卫）

> 源文件：`src/router/index.ts`

## 职责

定义全部路由（登录页 + 比赛工作台嵌套子树）与 `beforeEach` 守卫：首次导航统一恢复后端会话，受保护路由鉴权，并在**首次进入工作台（含深链）前校验比赛阶段**——赛前/赛后一律退回登录页等待。

## 核心类型/函数

| 名称 | 用途 |
|------|------|
| `RouteMeta` 声明扩展 | `title?` / `requiresAuth?`，避免 `to.meta.*` 退化为 any |
| 路由表 | `Login`(/login)；`Contest`(/contest, **requiresAuth**, redirect→ProblemSet, 组件 ContestLayout) 及子路由：`ProblemSet`(problems)、`ProblemSolve`(problem/:displayId)、`Rank`(rank)、`Submissions`(submissions)、`SubmissionDetail`(submissions/:submitId)、`Announcements`(announcements)、`Settings`(settings)；通配 `/:pathMatch(.*)*` → 重定向 /login |
| `router.beforeEach` | 会话守卫 + 赛程阶段闸门，见逻辑流程 |

## 直接依赖

- `vue-router`（`createRouter` / `createWebHistory` / 类型）
- `@/stores/authStore`（`sessionResolved` / `isLoggedIn` / `checkSession`）
- `@/stores/contestStore`（`brief` / `loadBrief` —— 阶段闸门的赛程来源）
- `@/utils/contest`（`getAuthoritativeSchedule` / `getContestPhase`）
- 全部视图组件（懒加载 `() => import(...)`）

## 被依赖

- `main.ts` — `app.use(router)` + 传给 `installSessionGuard`
- 各视图/组件的 `router.push/replace`（命名路由）

## 逻辑流程

```
beforeEach(to):
  1. !auth.sessionResolved → await auth.checkSession()
     首次导航统一向后端恢复会话（get_session）：既用于受保护路由鉴权，
     也让登录页首帧就渲染正确的会话状态，避免「先显示登录表单再跳转」的闪烁
  2. requiresAuth = to.matched.some(r => r.meta.requiresAuth)
     —— 子路由**不继承**父路由 meta，必须检查整条匹配链
  3. 非受保护 → 放行；受保护但未登录 → { name: 'Login', replace: true }
  4. 受保护且已登录，且当前不在 Contest 子树内
     （!router.currentRoute.matched 含 name === 'Contest'）时做赛程闸门：
       !contest.brief → await contest.loadBrief()    // 深链/首次进入补加载匿名简报
       期间会话可能失效 → !auth.isLoggedIn 则回 Login
       schedule = getAuthoritativeSchedule(contest.contest, contest.brief)
       getContestPhase(schedule, floor(Date.now()/1000)) !== 'running' → 回 Login
     否则放行
```

设计要点：

- **统一入口**：通配路由重定向 /login，由 LoginView 恢复会话并按比赛阶段决定是否进场
  （已登录且比赛进行中 → `canEnter` 侦听器自动 replace 到 Contest）。
- **新增赛程阶段闸门（PR #35）**：仅「已登录」不足以进入工作台 —— 深链 `/contest/...`
  在赛前/赛后同样退回登录页等待，到点由 LoginView 自动进场。否则选手可以把链接存下来
  在赛前直接打开工作台，看到空题目列表与错误的倒计时口径。
- **已在 Contest 子树内时跳过闸门**：工作台内切换页面由外壳的时钟处理；若每次都判阶段，
  比赛刚结束的瞬间用户在页面间跳转会被打断退出流程（结束弹窗/落盘收尾）。
- **权威赛程取 `getAuthoritativeSchedule`**：认证比赛与匿名简报 ID 不同即视为换场，
  以简报为准（见 `utils/contest.md`）。
- 守卫只做「有没有会话 + 比赛是否进行中」的粗判据；会话是否被服务端撤销由三态校验
  （赛前预检 `utils/session-check`）与全局 401 兜底（`guards/sessionGuard`）负责。
- `checkSession` 无论成败都置 `sessionResolved = true`（见 authStore），守卫不会每次
  导航重复发 IPC。
- 视图全部懒加载：首屏（登录页）不背负 Monaco/榜单等代码体积。

## 测试

`src/router/__tests__/contest-access.spec.ts` 在真实 router 上锁定（mock `createWebHistory → createMemoryHistory`，赛程 `startTime=1000` / `endTime=2000`）：赛前深链被拦回 Login、开赛左边界放行、赛后拦回 Login、认证态 `status=1` 优先于匿名简报、认证比赛与简报 ID 不同时采用简报、无会话仍走原会话守卫、简报缺失时补调一次 `loadBrief` 且失败后停在 Login。
