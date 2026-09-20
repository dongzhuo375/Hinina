# submissionStore（提交记录、评测轮询与提交历史）

> 源文件：`src/stores/submissionStore.ts`

## 职责

持有本次会话的提交条目，编排「提交 → 收敛轮询 → 终态停止」链路；并作为服务端**提交历史**（评测页 / 最新记录 pill）的状态源。终态判据复用 `utils/submission`，节奏参数来自 `configService`，轮询统一走 `utils/polling` 的 `createPoller`（P54 迁移）；失败原因（CE 等）随轮询回填到条目，轮询失败则按阈值上报（连续 3 次才提示，单次瞬时抖动保持静默）。

## 核心类型/函数

模块级 `pollContexts: Map<submissionId, { poller: Poller; deadline: number; failures: number }>` — 轮询器、截止时刻与连续失败计数是副作用句柄而非渲染状态，放模块作用域（不进响应式系统），由 `stopPolling` / `stopAllPolling` 统一回收。`POLL_ERROR_REPORT_THRESHOLD = 3` — 连续失败多少次才把提示打到界面（见设计要点）。

| 名称 | 签名 | 用途 |
|------|------|------|
| `SubmissionEntry` | interface（模块内） | `{ id, problemId, status, time?, memory?, submittedAt, errorMessage? }` —— 本会话提交条目；`time`(ms)/`memory`(KB)/`errorMessage` 由轮询回填 |
| state | `submissions` / `isSubmitting` / `error` / `history` | 本地提交列表、请求状态、服务端提交历史子状态 |
| `history` | 子对象 | `{ contestId, records, total, current, pageSize, pages, problemFilter, statusFilter, isLoading, error }` —— 评测页数据源（onlyMine 由后端强制） |
| `latestLocalFor` | getter `(problemId) => SubmissionEntry \| null` | 本题本会话最新一条提交 |
| `submitCode` | `(contestId, problemId, displayId, language, sourceCode) => Promise<string>` | 提交 → 追加 Pending 条目 → 启动轮询；失败记录 error 并抛出。**`problemId`（真实 pid）与 `displayId`（比赛内题号 "A"）必须都传** —— 各 OJ 认的不是同一个标识，见 `bridge/submission.bridge.md` |
| `pollResult` | `(submissionId) => Promise<JudgementResult>` | 单次查询并回填 `status`/`time`/`memory`/`errorMessage`；**成功即清 `error`**（P69：一次瞬时失败的红字不得永久残留）。**失败不再写 `error`** —— 上报改为阈值制（`notePollFailure`），单次瞬时抖动保持静默是刻意设计 |
| `startPolling` | `(submissionId) => Promise<void>` | 幂等启动：先停已有轮询器；读 `getPollSchedule()` 后建 `createPoller`（间隔抖动 ±20% 封顶 500ms），记入 `pollContexts`（`failures: 0`） |
| `pollOnce` | `(submissionId) => Promise<void>` | 超 deadline → warn + 停止；否则查询一次，成功即 `failures = 0`，`isTerminalStatus` 命中即停；`catch` 里 `notePollFailure`（**计数点必须在这里**，见设计要点） |
| `notePollFailure` | `(submissionId) => void` | 连续失败计数 +1；达 `POLL_ERROR_REPORT_THRESHOLD`(3) 时写 `error = \`评测结果查询连续失败 N 次（提交 #id），仍在重试\``；成功一次即清零 |
| `stopPolling` / `stopAllPolling` | `(submissionId?) => void` | 停单个 / 停全部（登出经 `stores/session` 调用） |
| `fetchHistory` | `(contestId, page?) => Promise<void>` | 拉一页提交历史，透传 `problemFilter`/`statusFilter`/`pageSize`；失败写 `history.error` 并抛出 |
| `setHistoryProblemFilter` / `setHistoryStatusFilter` | `(contestId, value) => Promise<void>` | 设置筛选并回到第 1 页 |
| `resetHistoryFilters` | `() => void` | 清空题目/状态筛选，**不发请求**——评测页无 `?problem=` 深链进入时清除上次访问残留（L2），首查由调用方随后的 `fetchHistory` 统一发出；与 setHistoryProblemFilter（用户改筛选、清空即重拉）语义不同 |
| `setHistoryPage` | `(contestId, page) => Promise<void>` | 翻页（越界不发请求） |
| `fetchProblemSummary` | `(contestId, displayId) => Promise<{ latest, total }>` | 只取 1 条，供解题页「最新记录」pill 与「提交记录 (n)」；**不污染 `history`** |

## 直接依赖

- `pinia`
- `@/types/submission`（`JudgementStatus` / `SubmissionRecord` 类型）
- `@/services/submission.service`（提交、查询、历史、详情）
- `@/services/config.service`（轮询间隔与总超时）
- `@/utils/submission`（`isTerminalStatus` 终态判据）
- `@/stores/problemStore`（终态到达**与轮询超时**时 `invalidateMyStatus()`：我的题目状态只由自己的提交改变、超时时终态未知，两种情形都由本 store 显式触发失效而非让总览页按周期整表重拉）
- `@/utils/polling`（`createPoller` 轮询原语）
- `@/utils/error`（`errorMessage` —— 错误文案收敛）
- `@/utils/logger`（`createLogger` —— 作用域日志）

## 被依赖

- `views/ProblemSolveView.vue` — `handleSubmit` 调 `submitCode(contestId, problem.id, displayId, language, code)`（`displayId` 取路由参数）
- `views/SubmissionsView.vue` — `history` / `fetchHistory` / 筛选 / 翻页
- `views/SubmissionDetailView.vue` — 详情与测试点（经 service）
- `components/editor/EditorConsoleBar.vue` — `latestLocalFor` / `fetchProblemSummary` 渲染最新记录 pill 与提交计数；`submissions[].errorMessage` 展示失败原因首行
- `components/problem/QuickSubmitDialog.vue` — `submitCode(contestId, problem.problemId, problem.displayId, language, code)`
- `stores/session.ts` — 登出清理：`stopAllPolling()` + `$reset()`

## 逻辑流程

```
submitCode(contestId, problemId, displayId, language, sourceCode)
  → submissionService.submitCode → submissionId
  → submissions.push({ id, problemId, status: 'Pending', submittedAt })
  → startPolling(submissionId)
     ├─ stopPolling(id)（幂等）
     ├─ getPollSchedule() → { intervalMs, timeoutMs }
     ├─ 条目不存在（会话已清理）→ 不注册
     └─ createPoller(task=pollOnce, intervalMs, jitterMs=min(500, 20%·interval)) → start

pollOnce(id)
  ctx = pollContexts.get(id)；无则返回
  now ≥ ctx.deadline → warn + stopPolling（总超时兜底）
  try:
    pollResult → 回填 status/time/memory/errorMessage + 清 error
    ctx.failures = 0
    isTerminalStatus → stopPolling
  catch:                        // 网络抖动等瞬时错误：等下一周期
    notePollFailure(id)         // ← 计数点在这里，不在 poller.onError（见设计要点）
```

设计要点：

- **P54 迁移到 createPoller**：递归 setTimeout + 逐周期抖动 + 重入保护，取代裸 `setInterval`。
- **节拍唯一归属前端**：后端 `get_judgement` 是单次查询（无内层阻塞循环），服务端请求节奏 = 本 store 的 Poller 周期，±20% 抖动真实生效；`stopPolling`（登出）即刻停发请求，不存在停不掉的在途后端循环。历史教训：曾经后端 `poll_judgement` 自带固定 2s 内层循环（阻塞到终态才返回），前端 Poller 的重入保护令后续 tick 全部空转 —— 抖动沦为装饰、真实节奏是后端固定间隔、`stopPolling` 停不掉在途循环，双层轮询已拆除。
- **刻意不配置 `isPaused`**：提交结果轮询是有限生命周期的收敛轮询，选手切窗口查资料回来就该看到结果；后台暂停只会推迟收敛、拉长「评测中」焦虑期（与榜单类无限轮询取舍相反）。
- **失败上报是阈值制（`POLL_ERROR_REPORT_THRESHOLD = 3`）**：偶发丢包/服务端瞬时 5xx 静默容忍（不打扰选手），真正断网时一个轮询周期内就给出反馈（默认节拍 1~2s，约 3~6s 可见）。阈值之前完全静默是刻意设计，但**永远静默**会让「服务端挂了」与「评测很慢」在界面上完全同形，选手只能干等 —— 所以 `pollResult` 不再在失败时写 `error`（否则一次瞬时抖动就闪一条红字），单次失败静默、连续失败才提示。
- **计数点必须在 `pollOnce` 的 `catch`，不能挂在 poller 的 `onError`**：`pollOnce` 自己吞掉异常，poller 永远收不到 rejection —— 把计数挂在 `onError` 上等于计数永不发生（错误提示静默失效）。`onError` 仅保留为防御性兜底（万一 `pollOnce` 真抛了，例如依赖的 store 初始化失败，也要记一次失败）。
- **`errorMessage` 随轮询回填**：CE 编译错误是选手改代码的唯一依据；没有它，解题页只显示「Compile Error」四个字，必须点进详情页才知道错在哪。控制台条只展示首行（完整多行在详情页），服务端占位文案已在 Rust 侧过滤。
- **pid 与 displayId 双标识**：`submitCode` 两者都要 —— 工作区隔离/状态查询用 pid，而 HOJ 的提交接口收的是比赛内展示题号，传数字 pid 会让服务端抛 NPE 返回 HTTP 500（Hydro 则收真实 ID，由 Adapter 各取所需）。
- **终态判据单一来源**：`utils/submission.isTerminalStatus`（与 Rust `is_terminal_status` 对齐，`Unknown` 视为终态）——store 不自建状态列表。
- **终态触发 myStatus 失效**：轮询拿到终态时调用 `useProblemStore().invalidateMyStatus()`，总览页据此在下次可见刷新时重拉一次我的题目状态 —— 该数据只由我自己的提交改变（他人 AC 不影响），按 30s 周期整表重拉纯属浪费；失败路径不触发（还会继续轮询）。
- **超时停止也必须失效**：deadline 分支（`pollTimeoutSecs`，默认 300s）同样调 `invalidateMyStatus()` —— 超时意味着**终态未知**（服务端可能稍后才出结果），安全动作就是让总览页重拉。开场判题积压时评测超过 5 分钟是现实场景，漏掉这一处置会让 AC/尝试过 pill 与解题进度一直错到用户进入某题或下次提交。
- **总超时兜底**：deadline（默认 300s）保证服务端一直返回非终态时也会停止。
- **句柄放模块级 Map**：进响应式系统会被 Vue 深层代理且 `$reset()` 清不掉；登出必须先 `stopAllPolling()` 再 `$reset()`。
- **history 与 fetchProblemSummary 隔离**：解题页 pill 的轻量查询不干扰评测页的列表/筛选/分页状态。
- **筛选不跨访问泄漏（L2）**：history 是模块级持久状态，评测页无深链筛选进入时须先 `resetHistoryFilters()` 再首查，否则上次访问的题目/状态筛选会静默过滤列表。

## 测试

`src/stores/__tests__/submissionStore.spec.ts`：终态停止、memory 回填、超时兜底、瞬时失败恢复后清 error（P69）、stopAllPolling 回收、startPolling 幂等、**submitCode 把 displayId 原样透传给 service（pid 与 displayId 必须都送到后端）**、**轮询回填失败原因到条目（CE 首行）与 AC 时为空**、**连续失败达阈值后提示到界面、恢复成功后自动清除**、fetchHistory 参数透传与错误、筛选回第 1 页、resetHistoryFilters（清空筛选且不发请求）、翻页越界、fetchProblemSummary（latest+total、空提交、不污染 history）、latestLocalFor、终态触发 `problemStore.invalidateMyStatus`、**轮询超时（终态未知）同样触发失效**。轮询用 `vi.useFakeTimers()` 推进。
