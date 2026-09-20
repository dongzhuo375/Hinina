# index（路由表与会话守卫）

> 源文件：`src/router/index.ts`

## 职责

定义全部路由（登录页 + 比赛工作台嵌套子树）与 `beforeEach` 会话守卫：首次导航统一恢复后端会话，受保护路由无会话时重定向登录页。

## 核心类型/函数

| 名称 | 用途 |
|------|------|
| `RouteMeta` 声明扩展 | `title?` / `requiresAuth?`，避免 `to.meta.*` 退化为 any |
| 路由表 | `Login`(/login)；`Contest`(/contest, **requiresAuth**, redirect→ProblemSet, 组件 ContestLayout) 及子路由：`ProblemSet`(problems)、`ProblemSolve`(problem/:displayId)、`Rank`(rank)、`Submissions`(submissions)、`SubmissionDetail`(submissions/:submitId)、`Announcements`(announcements)、`Settings`(settings)；通配 `/:pathMatch(.*)*` → 重定向 /login |
| `router.beforeEach` | 会话守卫，见逻辑流程 |

## 直接依赖

- `vue-router`（`createRouter` / `createWebHistory` / 类型）
- `@/stores/authStore`（`sessionResolved` / `isLoggedIn` / `checkSession`）
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
  3. 非受保护 → 放行；受保护 → isLoggedIn ? 放行 : { name: 'Login', replace: true }
     （登出后即使组件自身跳转失败，也不会滞留在比赛页面）
```

设计要点：

- **统一入口**：通配路由重定向 /login，由 LoginView 恢复会话并按比赛阶段决定是否进场
  （已登录且比赛进行中 → canEnter 侦听器自动 replace 到 Contest）。
- 守卫只做「有没有会话」的粗判据；会话是否被服务端撤销由三态校验（赛前预检
  `utils/session-check`）与全局 401 兜底（`guards/sessionGuard`）负责，导航层不重复实现。
- `checkSession` 无论成败都置 `sessionResolved = true`（见 authStore），守卫不会每次
  导航重复发 IPC。
- 视图全部懒加载：首屏（登录页）不背负 Monaco/榜单等代码体积。
