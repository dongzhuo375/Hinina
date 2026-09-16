# submissionStore（提交记录、评测轮询与提交历史）

> 源文件：`src/stores/submissionStore.ts`

## 职责

持有本次会话的提交条目，编排「提交 → 收敛轮询 → 终态停止」链路；并作为服务端**提交历史**（评测页 / 最新记录 pill）的状态源。终态判据复用 `utils/submission`，节奏参数来自 `configService`，轮询统一走 `utils/polling` 的 `createPoller`（P54 迁移）。

## 核心类型/函数

模块级 `pollContexts: Map<submissionId, { poller: Poller; deadline: number }>` — 轮询器与截止时刻是副作用句柄而非渲染状态，放模块作用域（不进响应式系统），由 `stopPolling` / `stopAllPolling` 统一回收。

| 名称 | 签名 | 用途 |
|------|------|------|
| `SubmissionEntry` | interface（模块内） | `{ id, problemId, status, time?, memory?, submittedAt }` —— 本会话提交条目；`time`(ms)/`memory`(KB) 由轮询回填 |
| state | `submissions` / `isSubmitting` / `error` / `history` | 本地提交列表、请求状态、服务端提交历史子状态 |
| `history` | 子对象 | `{ contestId, records, total, current, pageSize, pages, problemFilter, statusFilter, isLoading, error }` —— 评测页数据源（onlyMine 由后端强制） |
| `latestLocalFor` | getter `(problemId) => SubmissionEntry \| null` | 本题本会话最新一条提交 |
| `submitCode` | `(contestId, problemId, language, sourceCode) => Promise<string>` | 提交 → 追加 Pending 条目 → 启动轮询；失败记录 error 并抛出 |
| `pollResult` | `(submissionId) => Promise<JudgementResult>` | 单次查询并回填 `status`/`time`/`memory`；**成功即清 `error`**（P69：一次瞬时失败的红字不得永久残留） |
| `startPolling` | `(submissionId) => Promise<void>` | 幂等启动：先停已有轮询器；读 `getPollSchedule()` 后建 `createPoller`（间隔抖动 ±20% 封顶 500ms），记入 `pollContexts` |
| `pollOnce` | `(submissionId) => Promise<void>` | 超 deadline → warn + 停止；否则查询一次，`isTerminalStatus` 命中即停；瞬时失败静默等下一周期 |
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
- `@/utils/polling`（`createPoller` 轮询原语）

## 被依赖

- `views/ProblemSolveView.vue` — `handleSubmit` 调 `submitCode`
- `views/SubmissionsView.vue` — `history` / `fetchHistory` / 筛选 / 翻页
- `views/SubmissionDetailView.vue` — 详情与测试点（经 service）
- `components/editor/EditorConsoleBar.vue` — `latestLocalFor` / `fetchProblemSummary` 渲染最新记录 pill 与提交计数
- `components/problem/QuickSubmitDialog.vue` — `submitCode`
- `stores/session.ts` — 登出清理：`stopAllPolling()` + `$reset()`

## 逻辑流程

```
submitCode
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
  pollResult → 回填 status/time/memory + 清 error → isTerminalStatus → stopPolling
  抛错（网络抖动）→ 吞掉，等下一周期
```

设计要点：

- **P54 迁移到 createPoller**：递归 setTimeout + 逐周期抖动 + 重入保护，取代裸 `setInterval`。
- **刻意不配置 `isPaused`**：提交结果轮询是有限生命周期的收敛轮询，选手切窗口查资料回来就该看到结果；后台暂停只会推迟收敛、拉长「评测中」焦虑期（与榜单类无限轮询取舍相反）。
- **终态判据单一来源**：`utils/submission.isTerminalStatus`（与 Rust `is_terminal_status` 对齐，`Unknown` 视为终态）——store 不自建状态列表。
- **总超时兜底**：deadline（默认 300s）保证服务端一直返回非终态时也会停止。
- **句柄放模块级 Map**：进响应式系统会被 Vue 深层代理且 `$reset()` 清不掉；登出必须先 `stopAllPolling()` 再 `$reset()`。
- **history 与 fetchProblemSummary 隔离**：解题页 pill 的轻量查询不干扰评测页的列表/筛选/分页状态。
- **筛选不跨访问泄漏（L2）**：history 是模块级持久状态，评测页无深链筛选进入时须先 `resetHistoryFilters()` 再首查，否则上次访问的题目/状态筛选会静默过滤列表。

## 测试

`src/stores/__tests__/submissionStore.spec.ts`：终态停止、memory 回填、超时兜底、瞬时失败恢复后清 error（P69）、stopAllPolling 回收、startPolling 幂等、fetchHistory 参数透传与错误、筛选回第 1 页、resetHistoryFilters（清空筛选且不发请求）、翻页越界、fetchProblemSummary（latest+total、空提交、不污染 history）、latestLocalFor。轮询用 `vi.useFakeTimers()` 推进。
