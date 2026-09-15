# SubmissionDetailView（提交详情页）

> 源文件：`src/views/SubmissionDetailView.vue`

## 职责

单条提交的完整详情（路由 `SubmissionDetail`，`/contest/submissions/:submitId`）：判定横幅（大状态 pill + 指标条 + 元信息）、CE/错误信息面板（可复制）、测试点明细（平铺 / 子任务分组）、源代码只读查看（默认折叠）；评测未收敛时以配置驱动的收敛轮询自动刷新，终态或总超时即停。

## 核心类型/函数

常量：`JITTER_RATIO`(0.2)、`JITTER_CAP_MS`(500) —— 抖动 ±20% 间隔封顶 500ms，与 submissionStore 同口径。普通变量：`alive`（卸载标记）、`poller` / `deadline` / `pollIntervalMs` / `pollJitterMs`（副作用句柄与轮询参数，不进响应式系统）。

| 名称 | 签名 | 用途 |
|------|------|------|
| `submitId` | computed | 路由参数 `String(route.params.submitId)` |
| `detail` / `cases` | `ref<SubmissionDetail \| null>` / `ref<SubmissionCases \| null>` | **视图本地状态，不经 store**：详情页是一次性只读消费，无跨视图共享需求 |
| `casesError` / `loadError` / `isLoading` | ref | 测试点明细失败**不致命**（降级为提示条，主体照常展示）；详情失败仅在无旧数据时整页报错 |
| `judging` | computed | `isJudging(detail.status)`，驱动「评测中，自动刷新…」指示与骨架占位 |
| `matchedProblem` | computed | `detail.pid` 匹配当前比赛题目 → 可跳解题页；匹配不到（如赛后换配置）只显示 displayPid |
| `submitTimeText` / `contestElapsedText` | computed | 本地化提交时间；赛时相对时间（`formatDurationHms(submitTime − startTime)`） |
| `codeLanguage` | computed | `mapLanguageToMonaco(detail.language)` —— 服务端返回展示名，按前缀归一为 Monaco id |
| `caseList` / `acCaseCount` / `hasSubTasks` / `showScoreColumn` / `modeBadge` | computed | 测试点表派生：通过计数、子任务制判定（`subTasks` 非空）、得分列仅在有 score 时出现、判题模式徽章（非 `default` 才显示，如 spj/subtask） |
| `copyErrorMessage` | fn | 剪贴板复制 CE 信息；WebView API 不可用时兜底隐藏 textarea + `execCommand('copy')`；「已复制」1.5s 回弹 |
| `loadAll` / `retry` | — | `Promise.allSettled` 并行拉详情与测试点，见逻辑流程 |
| `syncPoller` / `stopPoller` | — | 收敛轮询启停决策 |
| `goBack` / `openProblem` | — | 返回评测列表（`Submissions` 路由）/ 跳解题页 |

## 直接依赖

- `vue` / `vue-router`
- 组件：`CodeEditor`（readonly 模式）/ `ErrorMessage` / `LoadingSpinner`
- `@/services/config.service`（`getPollSchedule` 轮询参数）、`@/services/submission.service`（`getSubmissionDetail` / `getSubmissionCases`）
- `@/stores/contestStore`（`whenLoaded`：赛时相对时间与题目跳转）
- `@/types/submission`（仅类型）
- `@/utils/submission`（`formatCodeLength` / `formatDurationHms` / `formatMemoryKb` / `isJudging` / `isTerminalStatus` / `mapLanguageToMonaco` / `statusAbbr` / `statusLabel` / `statusTone` + `StatusTone` 类型）
- `@/utils/polling`（`createPoller` + `Poller` 类型）

## 被依赖

- `router/index.ts` — 路由 `SubmissionDetail`
- 入口：`views/SubmissionsView.vue`（查看详情）、`components/editor/EditorConsoleBar.vue`（最新记录 pill 点击）、`components/problem/QuickSubmitDialog.vue`（终态后查看详情）

## 逻辑流程

```
onMounted:
  contestStore.whenLoaded().catch(静默)        // 深链直接进入时比赛数据可能未加载
  schedule = configService.getPollSchedule()   // intervalMs / timeoutMs（服务内部有兜底值）
  pollIntervalMs = schedule.intervalMs
  pollJitterMs   = min(500, round(intervalMs × 0.2))
  deadline       = now + schedule.timeoutMs
  loadAll()

loadAll():
  Promise.allSettled([getSubmissionDetail(id), getSubmissionCases(id)])
  详情成功 → 覆盖 detail、清 loadError
  详情失败 → 仅当**无旧数据**时写 loadError（轮询期间瞬时失败不清空页面）
  测试点失败 → casesError 提示条（主体不受影响）
  → syncPoller()

syncPoller():
  detail 非终态且 now < deadline 且无 poller → createPoller(task=loadAll, 不配置 isPaused)
  终态或超时 → stopPoller()                    // 收敛即停

onUnmounted: alive = false；stopPoller()；清理 copiedTimer
```

设计要点：

- **收敛轮询与 submissionStore 同一取舍**：不配置 `isPaused`（页面隐藏不暂停）——
  有限生命周期轮询，选手切窗口查资料回来就该看到结果；总超时（deadline）兜底防无限轮询。
- **详情/测试点并行且各自降级**：`allSettled` 保证一个失败不拖垮另一个；测试点明细
  在部分 OJ 配置下会被隐藏，终态无明细时提示「以最终判定为准」而非报错。
- 评测中且无明细时渲染 4 行脉冲骨架（渐进透明度），提示「测试点结果将自动刷新」。
- 子任务制按 `subTasks[].groupNum` 分组渲染，每组独立表头与「N/M 通过」计数；
  平铺制直接一张表。得分列（OI）仅在任一测试点带 score 时出现。
- 指标条中 `score` / `oiRankScore` 为 null（ACM 题）时整项不渲染，不显示假 0。
- 源代码卡默认折叠（赛场详情页主要看判定与测试点）；展开后复用 `CodeEditor`
  readonly 模式（隐藏工具条、禁用编辑与 Ctrl+Enter）。
- 分层例外说明：本视图直接消费 `submission.service`（一次性只读查询，无共享状态），
  提交/轮询等有状态链路仍走 store。
