# LoginView（登录页）

> 源文件：`src/views/LoginView.vue`

## 职责

应用统一入口页：左侧登录表单 / 已登录欢迎态 / 切换账号，右侧比赛氛围区（匿名简报 + 阶段徽章 + 天/时/分/秒倒计时）；负责会话恢复、按比赛阶段决策进场导航、以及等待开赛期间的错峰会话预检。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `brief` / `connState` / `briefError` | computed | 匿名简报三件套，全部来自 `contestStore`（登出/切换账号后依然保留） |
| `version` | `__APP_VERSION__` | 版本号构建期注入（vite define ← package.json），与 StatusBar 同源，避免写死漂移 |
| `now` / `countdown` / `countdownSegs` / `segColor` | ref/computed/fn | 每秒时钟 → 倒计时四段（天/时/分/秒，非零紫色、归零黑色） |
| `contestPhase` / `phaseLabel` / `phaseDesc` | computed | `utils/contest.getContestPhase`（与顶栏共用判据）→ 徽章与引导文案（已登录/未登录话术不同） |
| `canEnter` | computed | **`auth.isLoggedIn && hasContestStarted(phase)`** —— 比赛未开始恒 false，登录成功后留在本页等待倒计时归零 |
| `watch(canEnter)` | immediate | 导航决策唯一入口：true → `router.replace({ name: 'Contest' })`；倒计时归零使 phase 跃迁时本侦听器再次触发自动进场 |
| `handleLogin` | fn | `auth.login` 成功后**立即清除内存中的明文密码**（`password.value = ''`）；不直接跳转，进场交给 canEnter 侦听器 |
| `handleSwitchAccount` | fn | `auth.logout()`（永不 reject，无需捕获）+ 清密码；`isSigningOut` 防重复点击 |
| `enterContest` / `reloadBrief` | fn | 手动进场兜底（自动导航失败/简报加载失败时的入口）；简报重试 |
| `reschedulePrecheck` / `runPrecheck` / `clearPrecheck` | fn | 赛前会话预检调度（策略纯函数在 `utils/session-check`），见逻辑流程 |
| `renderedBrief` / `briefVisible` | computed/ref | 简介 Markdown 渲染（相对图片经 briefBaseUrl 改写）与折叠面板开关 |

模块级普通变量：`timer`（每秒时钟）、`precheckTimer`（预检排程句柄）、`precheckRetried`（unknown 只重试一次标记）——副作用句柄不进响应式系统。

## 直接依赖

- `vue` / `vue-router`
- `@/stores/authStore` / `@/stores/contestStore`
- `@/utils/markdown`（`renderMarkdown`）、`@/utils/contest`（`getContestPhase` / `hasContestStarted`）、`@/utils/session-check`（`planNextPrecheck` / `planRetryDelayMs` + `PrecheckReason` 类型）

## 被依赖

- `router/index.ts` — 路由 `Login`（/login，通配重定向目标）

## 逻辑流程

```
onMounted:
  reloadBrief()                          // 匿名简报（contestStore.loadBrief 三态）
  timer = setInterval(1s)                // 驱动倒计时与阶段跃迁
  !auth.sessionResolved → auth.checkSession()   // 路由守卫可能已确认过，跳过避免重复 IPC
  reschedulePrecheck()

canEnter 侦听器（immediate）:
  已登录 && 比赛已开始 → replace 到 Contest（T-0 进场不错峰，公平性要求）
  已登录 && 未开始 → 留在本页（欢迎态 + 倒计时），归零瞬间 phase 跃迁 → 侦听器再触发进场

赛前预检（仅「已登录 + upcoming + 有 startTime」时排程）:
  planNextPrecheck(now, startMs) → { delayMs, reason } | null
  到点 runPrecheck(reason) → auth.validateSession()
    ├─ unknown 且未重试过 → planRetryDelayMs()（2–5s）后重试一次（避免重试风暴）
    ├─ invalid → authStore 已就地清理会话 → 回到登录表单（error 展示统一失效文案）
    └─ reason === 'periodic' → 继续排程；window/immediate 为一次性，
       之后交给进场流程与全局 401 兜底
  watch([isLoggedIn, contestPhase]) → 登录/登出/简报到达/开赛都重新排程
onUnmounted → 清理时钟与预检定时器
```

设计要点：

- **导航决策集中在 canEnter 侦听器**：登录成功、会话恢复、倒计时归零三条路径都只改状态，
  不各自调 router——单一决策点避免竞态与重复导航；「进入赛场」按钮仅作自动导航失败时的
  手动兜底。
- **未开赛不进场**：登录成功后留在本页等待（右侧氛围区承担赛程信息），而不是进工作台
  看空题目列表；`phase === 'none'` 且简报加载失败时提供「重新加载比赛信息」与
  「直接进入赛场」双兜底入口。
- **预检错峰**：调度策略全部在 `utils/session-check` 纯函数（窗口内一次性随机取点、
  周期复检 ±60s 抖动），本页只负责执行与重排——把全场客户端的预检请求散布在时间窗口内，
  避免开赛前对 OJ 的同步尖峰。
- 登录成功即清空 `password` ref：明文密码在内存中的存活时间最小化（与 bridge 层
  「日志不记 args」同一安全口径）。
- 简报状态由 contestStore 持有：切换账号（logout）不清空，右侧氛围区与倒计时不闪断
  （见 `stores/contestStore.md` 的两类状态划分）。
