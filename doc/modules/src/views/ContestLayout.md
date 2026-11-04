# ContestLayout（比赛工作台外壳）

> 源文件：`src/views/ContestLayout.vue`

## 职责

比赛工作台的布局外壳：`TopBar`（顶栏，内嵌横向导航）+ `<router-view>` 子视图 + `StatusBar`（底栏）；挂载时拉取比赛数据、**按权威赛程刷新跨场缓存**、在比赛进行中启动公告轮询，并以每秒时钟驱动**赛程闸门**——赛程被移到未来时立即退回登录页，比赛结束时叠加结束覆盖层并在 2 秒后自动返回登录页。

**无会话最小壳**：`--settings`（管理方钥匙）会在无会话时经守卫直落设置路由（守卫条件豁免其鉴权，见 `router/index.md`），此时外壳退化为最小壳——自带 `data-tauri-drag-region` 拖动区标题栏（「Hinina 设置」+「返回登录」，`decorations: false` 下窗口移动全靠它）+ 内容区；**不渲染** TopBar/StatusBar（其数据全部依赖会话），`bootstrap` 直接跳过（无会话时认证拉取只会失败），不产生任何面向 OJ 的请求。判据是**三条件与**：钥匙启动 + 无会话 + 设置路由（`launchFlags.settings && !isLoggedIn && route === 'Settings'`）——绑路由防「正常登出/会话失效在 `replace(Login)` 异步守卫完成前闪最小壳、子视图重挂」（评审二），绑钥匙防「普通启动在设置页上登出时闪最小壳」（评审三），**刻意不含 `sessionResolved`**：存量过期会话的首个认证请求 401 会把它复位为 false，此刻正是最小壳的形态（评审四 P2-1）——普通启动**永远**不进最小壳。

## 核心类型/函数

`<script setup>`，无 props/emits：

| 名称 | 签名 | 用途 |
|------|------|------|
| `nowMs` / `now` | `useNowMs()` / computed（秒） | 共享毫秒墙钟派生秒，驱动 `phase` 跃迁；scope dispose 清理 interval |
| `phase` | computed | `getContestPhase(getAuthoritativeSchedule(contestStore.contest, contestStore.brief), now)` |
| `keyedSettings` | computed | `systemService.isKeyedSettings(route.name)`（`services/system.service`）—— 设置页各项豁免的**共同判据**（阶段闸门跳转与锁定的短路、最小壳）。赛程闸门只防「滞留在过期赛场看旧数据」，设置页只碰本地配置不在其射程内 |
| `contestGated` | computed | `!keyedSettings && (phase === 'ended' \|\| phase === 'upcoming')` —— 赛程闸门的**模板判定**（赛程遮罩 v-if、内容区 inert、TopBar navigation-disabled 三处同源）；钥匙模式设置页恒为 false（评审六） |
| `bareShell` | computed | `keyedSettings && !auth.isLoggedIn` —— 最小壳判据（三条件与）；**刻意不含 `sessionResolved`**：钥匙 + 存量过期会话启动时首个认证请求 401 会把它复位为 false，此刻正是最小壳的形态（评审四 P2-1） |
| `watch([phase, keyedSettings], …)` | — | **赛程闸门**（唯一退出决策点）：`upcoming` → 立即 `replace(Login)`；`ended` → `stopLive()` 后 `setTimeout(2_000)` 再 `replace(Login)`；每次先清 `exitTimer`。**keyedSettings 时只免跳转**——无会话（最小壳，评审四 P2-2）与有会话（评审五）两个分支都覆盖；`ended` 与 `upcoming` 的 `stopLive()` 清理**不能被豁免吞掉**（自然结束或 `running→upcoming` 回退后公告请求会一直打，评审八 P2-1、评审三十三 #5），并清 `liveFor` 以便回到 running 重启 |
| `bootstrap` | `() => Promise<void>` | 首行 `bareShell` 直接 return（跳过认证拉取与公告轮询）；否则 `whenLoaded()` → 跨场刷新 → `ensureLive()`，见逻辑流程 |
| `ensureLive` | `() => void` | 公告轮询的唯一启动点：`phase === 'running'` 且 `contest.id` 存在、且不是跨场未刷新（brief.id ≠ contest.id）时 `startLive(id)`；按 id 幂等。**阶段跃迁到 running 时也调用**（评审十九 #5：赛前打开的外壳在开赛后无需重挂即可补启轮询） |
| `backToLogin` | `() => void` | 最小壳的「返回登录」按钮 → `replace(Login)`（正常登录页，含匿名简报与重试 UI） |
| `exitTimer` | 定时器句柄 | 结束退出延迟；模块级普通变量，不进响应式系统（墙钟 interval 已归 `useNowMs`） |
| `disposed` | `boolean` | 卸载后阻断在途异步续跑 |

## 直接依赖

- `vue`（`computed` / `onMounted` / `onUnmounted` / `ref` / `watch`）/ `vue-router`（`useRouter` / `useRoute`）
- `@/components/layout/TopBar.vue` / `StatusBar.vue` / `WindowControls.vue`（极简壳头部的窗口控件）
- `@/stores/authStore`（`isLoggedIn` —— 最小壳判据之一；`sessionResolved` 已不在判据内）
- `@/stores/contestStore` / `@/stores/announcementStore`
- `@/services/system.service`（`systemService.isKeyedSettings()` —— 最小壳与阶段闸门跳转豁免的唯一判据）
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

watch([phase, keyedSettings], immediate):
  每次先清 exitTimer（避免 upcoming 之后又被旧的 ended 定时器踢走）
  keyed → 评审四 P2-2 + 评审五：**跳转豁免，清理不豁免**
          phase === 'ended' 或 'upcoming' → announcementStore.stopLive() 并清 liveFor
          （自然结束/赛程回退后公告请求会一直打，评审八 P2-1、评审三十三 #5；
           回到 running 时 ensureLive 重启）
  upcoming → router.replace(Login)                  // 赛程被移到未来：立即回入口等待
  ended    → announcementStore.stopLive()
             2s 后 router.replace(Login)            // 保留会话与工作区，让离页保存链路落盘

onUnmounted → disposed = true；清退出定时器；announcementStore.stopLive()（墙钟由 scope dispose 清理）

模板：
  根 .contest-theme（启用赛场 CSS 令牌）
  bareShell（最小壳 chrome）：header[data-tauri-drag-region]（「Hinina 设置」+「返回登录」）
  否则：TopBar（navigation-disabled=contestGated）
  内容区两形态**共享同一 router-view 实例**（:inert="contestGated" 的容器 +
  workspace-main）——最小壳↔完整外壳翻转（会话失效/登出）不重挂设置视图，
  未保存草稿得以保留（评审六）
  StatusBar 仅完整外壳渲染
  contestGated 时叠加覆盖层（role="alert" 卡片，文案随相位分叉）：ended → CONTEST CLOSED / 比赛已结束 / 感谢参赛，即将返回登录入口；upcoming → CONTEST NOT STARTED / 比赛尚未开始 / 比赛开始前暂不开放，即将返回登录入口。**upcoming 也要遮罩**：跳转是异步的，窗口期里灰屏无解释就是「灰了但不知道为什么」（阶段回退与导航延迟时窗口更长）
```

设计要点：

- **会话有效性不在本组件校验**：由路由守卫（`meta.requiresAuth` + 赛程闸门）统一保证，
  外壳只负责数据、布局与赛程退出。唯一例外是最小壳：守卫对「钥匙启动 + 设置路由」
  条件豁免鉴权，外壳以 `bareShell` 识别该形态并退化为最小壳，**不发起任何认证/OJ
  请求**。
- **最小壳必须自带拖动区**：App 的全局标题栏在 matched 含 Contest 时不渲染，
  窗口 `decorations: false`，无拖动区则窗口无法移动。
- **最小壳含窗口控件（评审十 #2 反转了 M1 的「不加 UI」裁定）**：窗口
  `decorations: false`，只靠 Alt+F4/任务栏关闭在维护场景不可接受。抽出共享
  `components/layout/WindowControls.vue`（最小化/最大化/关闭），TopBar 与最小壳
  共用同一实现，消除重复。
- **赛程闸门的钥匙短路覆盖两种会话形态（评审五补齐）**：短路由判据是
  `keyedSettings`（钥匙 ∧ 设置路由）而非 `bareShell`——后者要求无会话，漏掉
  「钥匙 + 会话有效 + 已结束/未开始赛程」的有会话分支（探针实测两种形态都被
  踢回登录页）。离开设置页时监听重评，闸门对比赛子路由照常生效（反向用例锁定）。
- **最小壳判据必须含路由条件（评审二抓出）**：`logout()` 成功后 `sessionResolved`
  保持 true、`user` 置空是同步的，重渲染先于 `replace(Login)` 的异步守卫完成——
  只判会话会让比赛页外壳在登出瞬间闪成最小壳并重挂子视图。绑定 Settings 路由
  后，比赛子路由的闪帧在结构上不可能。
- **还必须含钥匙条件（评审三）**：已登录维护者经隐藏入口在设置页上登出时，路由
  同样是 Settings——`systemService.isKeyedSettings(route.name)` 带钥匙条件，让**普通启动永远
  不进最小壳**。判据委托 `systemService` 的启动标志快照（守卫首航填充，早于任何
  组件挂载），同步读取不会把闪帧反转到钥匙启动的首帧。
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
  防止在覆盖层下继续操作（提交、切题）造成「已结束还在提交」的错觉。三者统一由
  `contestGated`（= 非钥匙设置页 且 phase 为 ended/upcoming）驱动——**钥匙模式下
  设置页不受闸门锁死**（评审六：只短路跳转不修 inert，设置页照样不可用）。
- **内容区跨形态共享实例（评审六）**：顶栏/状态条按形态切换，但 router-view 不进
  分支——重挂设置视图会丢未保存草稿（维护者编辑中遇 401 翻转 bareShell 的场景）；
  结构持久化由 spec 以 DOM 节点同一性锁定。
- **jsdom 的 inert 反射怪癖（测试）**：`el.inert = false` 会留下 `inert="false"`
  属性（按 HTML 规范布尔属性「存在即生效」，真实浏览器里 Vue 对 false 是移除
  属性）——断言经 `contentInert` 判读（属性缺失或值为 "false" 都算未锁定）。
- **公告轮询归外壳而非公告页**：未读红点徽标在所有页面可见，轮询必须与工作台同生命
  周期；公告页自身只声明 `isWatching`（「用户正在看」），标记已读由 `store.load` 在
  该标记为真且页面可见时统一完成（详见 `stores/announcementStore.md`）。
- 设置页也放在外壳内（子路由），切换时不丢失顶栏/状态条。
- 分层约束：View 只 import store 与组件，不触 `@/bridge`；钥匙判据经
  `@/services/system.service`（`systemService.isKeyedSettings()`）统一取得，View 不直接读
  `systemService.launchFlags`（属路由级瞬态数据，`systemService` 内部同步读取快照）。

## 测试

`src/views/__tests__/ContestLayout.spec.ts`（jsdom，mock `useRouter` / `useRoute` / `announcementStore` / `contestService.loadConfiguredContest` / `systemService.launchFlags` 快照，stub 三个 layout 子组件，赛程 `startTime=900 / endTime=1002`）锁定：到点后出现 `role="alert"` 的结束卡片（父容器含 `top-10`）、存在 `[inert]`、`stopLive` 恰好一次、`replace` 未立即调用，2 秒后 `replace({ name: 'Login' })` 恰好一次；服务端 `status=1` 提前结束时同样处理；赛程被移到未来时立即 `replace(Login)` 且 `startLive` 未被调用；「旧 id 已结束 contest + 新 brief」以简报为准、不出覆盖层；brief 指向新比赛时 `loadConfiguredContest` 被调用一次且 `startLive` 用新 contestId；卸载后推进 5 秒 `replace` 未被调用且无残留定时器。最小壳用例（钥匙 + `user=null` 且**不设** `sessionResolved`，锁 bareShell 不读它）：渲染「Hinina 设置」拖动区标题与「返回登录」、不出 TopBar/StatusBar、`loadConfiguredContest` 与 `startLive` 均未被调用、点击返回登录 `replace(Login)`；普通启动（无钥匙）+ 设置路由/比赛子路由两种登出过渡态均保持正常外壳；保留的已结束/未开始简报在最小壳下不触发赛程跳转（评审四 P2-2）；最小壳↔完整外壳翻转时内容区 DOM 节点同一（评审六，草稿保留的结构锁定）。
