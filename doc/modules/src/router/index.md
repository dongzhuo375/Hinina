# index（路由表与会话/赛程守卫）

> 源文件：`src/router/index.ts`

## 职责

钥匙豁免的判定一律经 `guards/keyed-settings-policy`（`isKeyedSettingsRoute`），不在守卫内联。定义全部路由（登录页 + 比赛工作台嵌套子树）与 `beforeEach` 守卫：首次导航统一恢复后端会话，受保护路由鉴权，并在**首次进入工作台（含深链）前校验比赛阶段**——赛前/赛后一律退回登录页等待。另承载 `--settings` 管理方钥匙的前端落点：首次导航落登录页时直落设置页，且设置页的免登录访问是守卫里的**条件豁免**（`systemService.isKeyedSettings(to.name)`）——meta 保持继承父级保护，普通模式下无钥匙访客直达设置页会被送回登录页、页面 401 也照常回登录页。

## 核心类型/函数

| 名称 | 用途 |
|------|------|
| `RouteMeta` 声明扩展 | `title?` / `requiresAuth?`，避免 `to.meta.*` 退化为 any |
| 路由表 | `Login`(/login)；`Contest`(/contest, **requiresAuth**, redirect→ProblemSet, 组件 ContestLayout) 及子路由：`ProblemSet`(problems)、`ProblemSolve`(problem/:displayId)、`Rank`(rank)、`Submissions`(submissions)、`SubmissionDetail`(submissions/:submitId)、`Announcements`(announcements)、`Settings`(settings，meta 继承父级保护 —— 免登录是守卫的条件豁免，见逻辑流程)；通配 `/:pathMatch(.*)*` → 重定向 /login |
| `router.beforeEach` | 会话守卫 + 赛程阶段闸门，见逻辑流程 |

## 直接依赖

- `vue-router`（`createRouter` / `createWebHistory` / 类型）
- `@/services/system.service`（`getLaunchFlags` —— `--settings` 初始直落的标志来源，进程内 memoize + 失败退化「无参数」）
- `@/stores/authStore`（`sessionResolved` / `isLoggedIn` / `checkSession`）
- `@/stores/contestStore`（`brief` / `loadBrief` —— 阶段闸门的赛程来源）
- `@/utils/contest`（`getAuthoritativeSchedule` / `getContestPhase`）
- 全部视图组件（懒加载 `() => import(...)`）

## 被依赖

- `main.ts` — `app.use(router)` + 传给 `installSessionGuard`
- 各视图/组件的 `router.push/replace`（命名路由）

## 逻辑流程

```
beforeEach(to, from):   // 来源路由在入口一次性捕获，异步后不再读 currentRoute（评审二十一 #5）
  0. flags = await systemService.getLaunchFlags()（进程内 memoize，只真正发一次 IPC；
     try/catch 包死 —— service 已兜底，这里是导航路径上的第二道防线，守卫抛错=路由卡死）
     首次导航（模块级 firstNavigation 标记）且 flags.settings 且 to 为登录页
       → { name: 'Settings' }   // 管理方钥匙：初始直落设置页；仅首次，「返回登录」不受影响
  1. !auth.sessionResolved → await auth.checkSession()
     首次导航统一向后端恢复会话（get_session）：既用于受保护路由鉴权，
     也让登录页首帧就渲染正确的会话状态，避免「先显示登录表单再跳转」的闪烁
  2. requiresAuth = to.meta.requiresAuth === true
     —— 判据是**合并后的 to.meta**：vue-router 沿匹配链把父级 meta 合并进子级
     （子路由同键覆盖父路由），取最深层的值；未声明即不受保护（login/catch-all）
  3. 非受保护 → 放行；受保护但 `systemService.isKeyedSettings(to.name)` → 放行（**条件豁免**，
     不限首次——返回登录后想再修一次地址仍可直达）；受保护但未登录 → Login
     普通模式（无钥匙）设置页照常受保护：无钥匙访客直达 → Login；页面上 401 →
     sessionGuard 读到 requiresAuth=true 照常回登录页
  3a. 钥匙设置页出发、无会话：回 **Settings** 并提示「未登录」（评审二十——无条件 Login 会
      卸载设置视图、丢草稿）；
  4a. loadBrief 期间会话失效：钥匙流程回 **Settings**（评审十九 #2，与后续闸门失败同向）；
      简报缺失（服务器不可达，正是钥匙要修的场景）时提示「未能加载比赛信息」而非
      「比赛未进行」（评审十九 #3）
  4. 受保护且已登录，且「从钥匙设置页出发」或「当前不在 Contest 子树内」时
     做赛程闸门（前者是评审十 #1：从设置页点题目/榜单也要过闸门，否则会落进
     被锁定的比赛页、随后被外壳踢回登录页）：
       !contest.brief → await contest.loadBrief()    // 深链/首次进入补加载匿名简报
       期间会话可能失效 → 钥匙流程回 Settings（并提示「会话已失效」），其它回 Login
       schedule = getAuthoritativeSchedule(contest.contest, contest.brief)
       非 running → 钥匙流程回 Settings（提示按成因区分），其它回 Login
     否则放行
```

落地不变量（afterEach）：

```
afterEach(to):
  to.meta.requiresAuth !== true → return（登录页/catch-all 不受保护，不成环）
  已登录 → return
  systemService.isKeyedSettings(to.name) → return（钥匙设置页豁免）
  否则 → replace(Login)
```
**为什么需要**（评审九 P2-2）：beforeEach 只能看导航开始时的会话；守卫批准后、
路由确认前，某个在途请求返回 401 → sessionGuard 清理会话——它读到的
`currentRoute` 仍是**尚未确认**的旧路由（设置页），钥匙豁免会跳过跳转，豁免
被这条在途的受保护导航继承，用户落到无会话的比赛页。afterEach 在确认后补一次
同一判据（判据为 `systemService.isKeyedSettings(to.name)`），任何「导航中途会话失效」
都兜得住；替换目标是登录页（不受保护），不会循环。

设计要点：

- **统一入口**：通配路由重定向 /login，由 LoginView 恢复会话并按比赛阶段决定是否进场
  （已登录且比赛进行中 → `canEnter` 侦听器自动 replace 到 Contest）。
- **`--settings` 直落只在首次导航**：之后正常路径全部照旧（返回登录、隐藏入口、
  阶段闸门不受影响）。已登录用户带参启动同样直落设置页（管理方意图明确）。
- **豁免必须挂条件而不是写 meta**：无条件豁免（`requiresAuth: false`）会让无钥匙
  访客直达设置页，且 401 时 sessionGuard 读到 `requiresAuth=false` 不再回登录页，
  用户滞留在无会话的外壳里（评审 P2）。meta 继承保护 + 守卫条件豁免后，普通/
  钥匙两种模式的差异收敛为一处判据。
- **已登录用户经隐藏入口进设置页会跳过赛程阶段闸门**（闸门只挂在受保护链上，
  且从工作台内切换时 currentRoute 已在 Contest 子树）—— 偏好页在赛前/赛后可见
  属可接受行为；连接与缓存字段仍由分层锁保护（见 `views/SettingsView.md`）。
- **新增赛程阶段闸门（PR #35）**：仅「已登录」不足以进入工作台 —— 深链 `/contest/...`
  在赛前/赛后同样退回登录页等待，到点由 LoginView 自动进场。否则选手可以把链接存下来
  在赛前直接打开工作台，看到空题目列表与错误的倒计时口径。
- **已在 Contest 子树内时跳过闸门**：工作台内切换页面由外壳的时钟处理；若每次都判阶段，
  比赛刚结束的瞬间用户在页面间跳转会被打断退出流程（结束弹窗/落盘收尾）。
- **权威赛程取 `getAuthoritativeSchedule`**：认证比赛与匿名简报 ID 不同即视为换场，
  以简报为准（见 `utils/contest.md`）。
- 守卫只做「有没有会话 + 比赛是否进行中」的粗判据；会话是否被服务端撤销由三态校验
  （赛前预检 `utils/session-check`）与全局 401 兜底（`guards/sessionGuard`）负责。
- 只有取得明确会话结果才置 `sessionResolved = true`，守卫不会每次导航重复发 IPC；
  传输失败保持 unresolved，后续导航或登录页挂载可重试（见 authStore）。
- 视图全部懒加载：首屏（登录页）不背负 Monaco/榜单等代码体积。

## 测试

`src/router/__tests__/contest-access.spec.ts` 在真实 router 上锁定（mock `createWebHistory → createMemoryHistory`，赛程 `startTime=1000` / `endTime=2000`）：赛前深链被拦回 Login、开赛左边界放行、赛后拦回 Login、认证态 `status=1` 优先于匿名简报、认证比赛与简报 ID 不同时采用简报、无会话仍走原会话守卫、简报缺失时补调一次 `loadBrief` 且失败后停在 Login。

`src/router/__tests__/index.spec.ts` 用真实 routes 的 `router.resolve()` 锁定 requiresAuth **合并矩阵**：Settings 继承父级 true（免登录是守卫的条件豁免，不是 meta 声明）、其余工作台子路由继承父级 true、Login 无声明即不受保护 —— 判据本身是一行 `=== true`，会漂移的是「哪条路由声明了什么 meta」。

`src/router/__tests__/launch-redirect.spec.ts` 在真实 router 上锁定钥匙的**守卫行为**（mock `getLaunchFlags`，`resetModules` 保证每个用例拿到全新 `firstNavigation`）：带参首航直落、仅首航生效（再次回登录页不被重定向——不困死维护人员）、非登录页首航不直落、IPC 失败按「无参数」退化、无钥匙直达设置页回 Login（P2 路径一）、钥匙模式非首航直达设置页放行（豁免不限首次）。
