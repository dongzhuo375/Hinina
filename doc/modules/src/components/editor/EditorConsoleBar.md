# EditorConsoleBar（编辑器底部控制台条）

> 源文件：`src/components/editor/EditorConsoleBar.vue`

## 职责

编辑器下方状态区：最新评测记录 pill（**服务端最新记录优先，本会话提交回退**，点击进提交详情）、失败记录的首个非 AC 测试点提示、提交失败信息、「提交记录 (n)」入口（携带本题筛选跳评测页）与「提交代码」按钮、光标位置 / 编码 / 缩进状态行。

## 核心类型/函数

**props**：`cursor: { line, column } | null`（Monaco 光标，由 CodeEditor 经父级转发；null = 尚未产生光标事件）、`problemId: string | null`（当前题 pid，用于筛「本题最新一条」）、`tabSize?: number`（编辑器当前 Tab 宽度，默认 `utils/editor.DEFAULT_EDITOR_TAB_SIZE`；解题页弹层可改，故不写死）。
**emits**：`submit: []`。

| 名称 | 签名 | 用途 |
|------|------|------|
| `displayId` | computed | 当前路由的比赛内展示题号（`route.params.displayId`，解题页 `/contest/problem/:displayId`），摘要查询与 `?problem=` 深链的参数 |
| `latest` | computed | 从 `submissionStore.submissions`（只存本会话提交）尾部倒序找本题最新一条；服务端摘要未到达时的**回退数据源** |
| `summary` | `ref<{ latest: SubmissionRecord \| null; total: number } \| null>` | 服务端题目提交摘要，经 `submissionStore.fetchProblemSummary(contestId, displayId)`（limit=1 取最新一条 + total 计数）；独立于评测页 history state |
| `refreshSummary` | fn | 拉取摘要；失败仅 `log.warn` 记录（`utils/logger` 作用域日志）——**摘要失败不影响解题**，回退本地会话 pill |
| `failedCaseHint` / `hintSubmitId` | ref/普通变量 | 失败记录的测试点提示（如 `Test 4 · 2.01s`）；经 `findFirstFailedCase` 定位首个非 AC 测试点（**子任务制感知**：平铺 `cases` 为空时展开 `subTasks` 查找）；`hintSubmitId` 去重保证**每条提交只拉一次**测试点明细 |
| `serverPill` | computed | 服务端 pill 组装：`最新记录: #123 WA (Test 4 · 2.01s)`；测试点不可得时回退记录自身耗时（wa/tle 用 `formatMsToSeconds` 秒口径，其余 `N ms`） |
| `SERVER_TONE_MAP` | 常量 | `StatusTone`（utils/submission 语义色）→ 控制台条既有 `Tone` |
| `toneOf(status)` | `(JudgementStatus) => Tone` | 本地 pill 配色：AC=success；TLE/MLE=warning；Pending/Compiling/Running=pending（旋转圈）；Unknown=muted；WA/RE/CE=error |
| `TONE_STYLES` / `toneStyle` / `serverToneStyle` | — | Tone → global.css 语义色变量（fg/bg/border） |
| `statusLabel(status)` | 组件私有 fn | 本地 pill 文案：驼峰拆空格 `WrongAnswer → Wrong Answer`，**不缩写**——原词与判题语义一一对应（服务端 pill 则用 `statusAbbr` 紧凑缩写） |
| `timeLabel` | computed | 本地条目轮询回填的耗时（`entry.time`，ms）；未到终态无值不显示 |
| `totalCount` | computed | `summary.total`（「提交记录 (n)」计数；摘要未到达/为 0 时不显示 n） |
| `goSubmissions` / `goDetail` | fn | 跳 `Submissions`（携带 `?problem=displayId`）/ `SubmissionDetail` |

状态行：`UTF-8`（工作区文件由 Rust 后端以 UTF-8 落盘）、`Spaces: {{ tabSize }}`（Monaco 未关 insertSpaces，缩进恒为空格；宽度取编辑器当前 tabSize，由 CodeEditor 经父级转发，弹层可改故不写死）、`Ctrl + Enter 快捷提交` 快捷键提示。

## 直接依赖

- `vue` / `vue-router`
- `@/services/submission.service`（`getSubmissionCases` — 失败测试点提示）
- stores：`contestStore`（contestId）、`submissionStore`（`submissions` / `error` / `isSubmitting` / `fetchProblemSummary`）
- `@/types/submission`（仅 `JudgementStatus` / `SubmissionRecord` 类型）
- `@/utils/submission`（`findFirstFailedCase` / `formatMsToSeconds` / `isTerminalStatus` / `statusAbbr` / `statusTone` + `StatusTone` 类型）
- `@/utils/editor`（`DEFAULT_EDITOR_TAB_SIZE` — tabSize prop 兜底）
- `@/utils/logger`（`createLogger` —— 作用域日志）

## 被依赖

- `views/ProblemSolveView.vue` — 右栏编辑器下方（`:cursor` + `:problem-id` + `:tab-size`，submit 事件与 CodeEditor 共用同一 `handleSubmit`）

## 逻辑流程

```
摘要刷新触发点（三处）：
  onMounted                                → refreshSummary()
  watch [displayId, problemId]（切题/Tab 切换）→ 清空 summary/failedCaseHint 后重拉
  watch latest.status 到达终态              → refreshSummary()
      // 覆盖「快捷提交/本页提交收敛 → pill 换成服务端最新记录」的链路

watch summary.latest（服务端最新记录变化）:
  同一 submitId 已拉过测试点 → 跳过（hintSubmitId 去重）
  statusTone ∈ {wa, tle}（失败判定）
    → submissionService.getSubmissionCases(submitId)
    → findFirstFailedCase（平铺 cases 优先；子任务制 cases 为空时按 groupNum/seq
      展开 subTasks）→ failedCaseHint = `Test {seq} · {formatMsToSeconds(timeMs)}`
    → 失败 → log.warn 记录，pill 回退记录自身耗时

pill 优先级：serverPill（#id + statusAbbr + 测试点/耗时）
           > latest（本会话提交，原词文案 + ms 耗时）
           > 留空（不显示假数据）
点击 pill → SubmissionDetail；「提交记录 (n)」→ Submissions?problem=displayId
submissionStore.error 非空 → pill 旁截断展示失败原因（title 悬浮全文）
提交按钮：isSubmitting 时禁用 + 「提交中…」旋转圈，点击 emit submit
```

设计要点：

- **服务端摘要优先、本地会话回退**：摘要在途/拉取失败时展示本会话提交记录，
  两者皆无则留空——显示过期数据比留空更危险。本地提交到达终态后主动刷新摘要，
  使 pill 平滑切换到服务端口径（含失败测试点提示）。
- **失败测试点提示每条提交只拉一次**：`hintSubmitId` 去重，避免摘要反复刷新时
  重复打测试点接口；测试点不可得时回退记录自身耗时，pill 不留空洞。
- **子任务制感知（L1）**：subtask 判题下平铺 `cases` 常为空（明细在
  `subTasks[].cases`），`findFirstFailedCase`（utils/submission 纯函数）先查平铺、
  为空再按 groupNum/seq 展开子任务，避免「Test N」提示静默消失。
- 摘要查询走 `submissionStore.fetchProblemSummary`（独立于评测页 history，不干扰
  其列表/筛选）；测试点明细是组件级一次性只读查询，直接消费 submission.service。
- 色彩语义与 global.css 变量对齐，与榜单/题目卡片/评测页的状态色体系一致
  （`StatusTone` → 控制台条 `Tone` 经 `SERVER_TONE_MAP` 桥接）。
