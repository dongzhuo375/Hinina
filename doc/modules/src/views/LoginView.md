# LoginView（登录页）

> 源文件：`src/views/LoginView.vue`

## 职责

应用统一入口页：左侧登录卡片（`LoginForm`）承载表单 / 已登录欢迎态 / 切换账号，右侧氛围区由 `EventTitle`（比赛标题）与 `Countdown`（赛程时钟）组成，页脚为服务器连接状态与版本号；负责会话恢复、按比赛阶段决策进场导航、匿名简报的有界重试，以及等待开赛期间的错峰会话预检。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `username` / `password` | `ref<string>` | 登录表单双向绑定（传给 `LoginForm` 的 `v-model:*`） |
| `isSubmitting` / `isSigningOut` | `ref<boolean>` | 提交中 / 退出中标记（防重复点击，透传为表单禁用态） |
| `briefVisible` | `ref<boolean>` | 简介折叠面板开合 |
| `brief` / `connState` / `briefError` | computed | 匿名简报三件套，全部来自 `contestStore`（`brief` / `briefState` / `briefError`），登出/切换账号后依然保留 |
| `version` | `__APP_VERSION__` | 版本号构建期注入（vite define ← package.json），与 StatusBar 同源，避免写死漂移 |
| `now` | `ref<number>`（秒） | 每秒刷新，驱动倒计时与阶段跃迁 |
| `schedule` | computed | `getAuthoritativeSchedule(contestStore.contest, brief)` —— 权威赛程（认证态与匿名简报 ID 不同即取简报） |
| `contestPhase` | computed | `getContestPhase(schedule, now)`（与路由守卫、顶栏共用判据） |
| `eventTitle` | computed | `brief?.title \|\| 'Hinina 竞赛'` |
| `canEnter` | computed | **`auth.isLoggedIn && contestPhase === 'running'`** —— 仅进行中自动进场；`upcoming` 留在本页等待，`ended` 也不进场 |
| `routeIntoContest` | `() => void` | `router.replace({ name: 'Contest' })` |
| `watch(canEnter, …)` | immediate | **导航决策唯一入口**：true → 进场；倒计时归零使 phase 跃迁到 `running` 时本侦听器再次触发 |
| `handleLogin` | `async fn` | 空值/提交中短路 → `auth.login(username.trim(), password)`；成功后**立即清空明文密码**（`password.value = ''`）；不直接跳转，进场交给 `canEnter` 侦听器 |
| `handleSwitchAccount` | `async fn` | `auth.logout()`（永不 reject，无需捕获）+ 清密码；`isSigningOut` 防重入 |
| `renderedBrief` | computed | `renderMarkdown(brief?.description ?? '', contestStore.briefBaseUrl)`（相对图片 URL 改写为 OJ 绝对地址） |
| `loadBrief` / `retryBrief` | fn | 加载简报 / 手动重试（清定时器 + 重试计数归零后重拉） |
| `clearBriefRetry` | fn | 清简报重试定时器并复位 `briefRetryScheduled` |
| `watch(connState, …)` | — | 简报失败的有界重试调度（见逻辑流程） |
| `reschedulePrecheck` / `runPrecheck` / `clearPrecheck` | fn | 赛前会话预检的排程/执行/取消（策略纯函数在 `utils/session-check`） |
| `watch([() => auth.isLoggedIn, contestPhase], reschedulePrecheck)` | — | 登录态或阶段变化即重新排程 |

模块级普通变量（副作用句柄与计数，刻意不进响应式系统）：`disposed`（卸载标记，阻止卸载后在途回调继续排程）、`briefRetryCount` / `briefRetryTimer`（简报重试）、`precheckTimer` / `precheckRetried`（预检与 unknown 单次重试标记）。墙钟 interval 已归 `useNowMs`。另有 `briefRetryScheduled: ref<boolean>` 仅用于页脚文案（「正在自动重试」）。

## 直接依赖

- 字体：`@fontsource-variable/noto-serif-sc/wght.css`（标题衬线）、`@fontsource-variable/roboto-mono/wght.css`（倒计时等宽）——本地打包，不经 CDN
- `vue` / `vue-router`
- 组件：`@/components/login/EventTitle.vue` / `Countdown.vue` / `LoginForm.vue`
- stores：`@/stores/authStore` / `@/stores/contestStore`
- `@/utils/markdown`（`renderMarkdown`）、`@/utils/contest`（`getAuthoritativeSchedule` / `getContestPhase`）、`@/utils/brief-retry`（`briefRetryDelayMs`）、`@/utils/session-check`（`planNextPrecheck` / `planRetryDelayMs` + `PrecheckReason` 类型）
- `@/composables/useNowMs`（毫秒墙钟，派生秒）

## 被依赖

- `router/index.ts` — 路由 `Login`（/login，通配重定向目标）

## 逻辑流程

```
onMounted:
  loadBrief()                            // 匿名简报（contestStore.loadBrief 三态）
  !auth.sessionResolved → auth.checkSession()   // 路由守卫可能已确认过，跳过避免重复 IPC
  !disposed → reschedulePrecheck()

canEnter 侦听器（immediate）:
  已登录 && 比赛进行中 → replace 到 Contest（T-0 进场不错峰，公平性要求）
  未开始 → 留在本页（欢迎态 + 倒计时），归零瞬间 phase 跃迁 → 侦听器再触发进场
  已结束 → 不进场（留在入口页，表单区显示「比赛已结束，你已返回入口页面」）

简报重试 watch(connState):
  connected / unconfigured → clearBriefRetry + briefRetryCount = 0（停止重试）
  failed → delay = briefRetryDelayMs(briefRetryCount)   // 2s → 4s → 8s
           delay === null → 不再重试（保留「服务器连接失败」+ 手动「重试连接」）
           否则 briefRetryCount++、briefRetryScheduled = true、setTimeout(delay, loadBrief)

赛前预检（仅「已登录 + upcoming + 有 startTime」时排程）:
  planNextPrecheck(Date.now(), startSecs * 1000) → { delayMs, reason } | null
  到点 runPrecheck(reason) → auth.validateSession()
    ├─ unknown 且 !precheckRetried → precheckRetried = true，planRetryDelayMs()（2–5s）后重试一次
    ├─ invalid → authStore 已就地清理会话 → 回到登录表单（error 展示统一失效文案）
    └─ reason === 'periodic' → 继续排程；window/immediate 为一次性，
       之后交给进场流程与全局 401 兜底
  每次回调先查 disposed / isLoggedIn / phase，离开页面或开赛后不再排程
onUnmounted → disposed = true；清理简报重试与预检定时器；墙钟由 scope dispose 清理
```

设计要点：

- **导航决策集中在 `canEnter` 侦听器**：登录成功、会话恢复、倒计时归零三条路径都只改状态，
  不各自调 router —— 单一决策点避免竞态与重复导航。
- **只有进行中才进场**（PR #35 收紧）：旧实现用 `hasContestStarted`（running ∨ ended），
  已结束的比赛也会把选手送进工作台；现在 `ended` 停在入口页并明示「比赛已结束」。
- **权威赛程解决跨场/过期**：`schedule` 取 `getAuthoritativeSchedule`，认证比赛与匿名简报
  ID 不同即视为换场并以简报为准 —— 否则会拿上一次会话的旧赛程判阶段（详见 `utils/contest.md`）。
- **简报重试有界**（最多 3 次，2s/4s/8s）：登录页是应用入口，无限重试会在 OJ 不可达时
  持续打请求；用尽后保留错误文案与手动「重试连接」入口，`retryBrief` 会重置重试预算。
- **预检错峰**：调度策略全部在 `utils/session-check` 纯函数（窗口内一次性随机取点、
  周期复检 ±60s 抖动），本页只负责执行与重排 —— 把全场客户端的预检请求散布在时间窗口内，
  避免开赛前对 OJ 的同步尖峰。
- 登录成功即清空 `password` ref：明文密码在内存中的存活时间最小化（与 bridge 层
  「日志不记 args」同一安全口径）。
- **简报状态由 contestStore 持有**：切换账号（logout）不清空，右侧氛围区与倒计时不闪断
  （见 `stores/contestStore.md` 的两类状态划分）。
- **登录页拥有独立字体系统**（`--login-font-body` / `--login-font-title` / `--login-font-mono`，
  由 `@fontsource-variable/*` 本地打包）：入口页的文化气质不随工作台的全局字体调整而改变；
  文本选中色统一为木色 `#8c6a43`，覆盖浏览器默认紫色高亮。
- 页脚状态行是**真实状态**而非装饰：`connState` 五态（idle / connecting / connected /
  failed / unconfigured）各有独立文案，失败时同时给出原因（`briefError`）与重试入口 ——
  赛前连不上服务器时选手需要知道是「地址没配」还是「网络不通」。

## 测试

`src/views/__tests__/LoginView.spec.ts`（jsdom + fake timers，mock `vue-router` / `auth.service` / `contest.service`）锁定：会话恢复后到点自动进场 `replace({ name: 'Contest' })` 恰好一次、登录错误可访问（`role=alert` + `aria-invalid`）与成功后清空密码、提交期间禁用与 `aria-busy`、用户名 trim 后传参、简介 Markdown 相对图片改写、简报失败在 2/4/8 秒重试共 4 次后停止且不再打扰、手动「重试连接」重启重试周期、卸载后取消重试、已结束比赛不自动进场、缓存的已结束 contest 与新 ID 的 running brief 时采用简报进场、赛前预检调用一次且离开页面后不再重试。
