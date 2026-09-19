# submission（提交与评测跨端契约类型）

> 源文件：`src/types/submission.ts`

## 职责

评测状态、评测结果、提交历史/详情/测试点的跨端契约类型，与 Rust `core::entity::submission` 对齐。

## 核心类型/函数

| 名称 | 形状 | 关键语义 |
|------|------|----------|
| `JudgementStatus` | 20 值字符串联合（`NotSubmitted / Cancelled / Pending / Compiling / Running / Accepted / WrongAnswer / TimeLimitExceeded / MemoryLimitExceeded / RuntimeError / CompilationError / PresentationError / OutputLimitExceeded / SystemError / RemoteJudgeError / SubmitFailed / PartiallyAccepted / FrequentLimit / UnknownError / Unknown`） | OJ 无关的归一状态，**覆盖 HOJ 全部状态码**；变体名即 serde 序列化字符串，两端必须同步演进；无法识别的状态码归 `Unknown`。注意值域是 HOJ `Constants.Judge` 的原始码、**含负数**（`NotSubmitted` = -10 / `Cancelled` = -4 / `WrongAnswer` = -1 / `CompilationError` = -2 / `PresentationError` = -3），不是「0 起顺排」 |
| `JudgementResult` | `{ status, score, timeMs, memoryKb, errorMessage\|null }` | 单次评测查询结果（轮询投影）；**`timeMs` 毫秒 / `memoryKb` KB**；`errorMessage` 为失败原因（CE/SE/SF 时非空，服务端占位文案已在 Rust 侧过滤） |
| `SubmissionRecord` | `{ submitId, pid, displayPid, title, displayId, username, submitTime, status, timeMs, memoryKb, score\|null, length, language }` | 提交列表行（来源 HOJ JudgeVO）；`submitTime` 为 **epoch 秒**（与 `Contest.startTime` 同口径）；`displayId` 依赖请求带 `completeProblemID=true` 回填，可能为空串 |
| `SubmissionPage` | `{ records, total, size, current, pages }` | 提交列表分页结果 |
| `SubmissionDetail` | Record 字段 + `{ code, errorMessage\|null, judger\|null, oiRankScore\|null }` | 提交详情（来源 get-submission-detail）；CE 时 `errorMessage` 为编译错误输出 |
| `JudgeCase` | `{ caseId, seq, status, timeMs, memoryKb, score\|null, groupNum\|null }` | 单测试点结果；`seq` 从 1 开始 |
| `SubTaskCases` | `{ groupNum, cases }` | OI 子任务分组 |
| `SubmissionCases` | `{ cases, subTasks, mode }` | 测试点详情聚合（来源 get-all-case-result）；`mode` 为判题模式（default/spj/subtask） |
| `SubmissionListQuery` | `{ contestId, currentPage, limit, problemDisplayId?, status? }` | 列表查询参数；**onlyMine 不在其中** —— 「只显示本人」由 Rust 命令层强制，前端不可绕过 |

## 直接依赖

无（纯类型声明文件）

## 被依赖

- `bridge/submission.bridge.ts`、`services/submission.service.ts`、`stores/submissionStore.ts`、`utils/submission.ts`
- `views/SubmissionsView.vue`、`views/SubmissionDetailView.vue`、`components/editor/EditorConsoleBar.vue`

## 逻辑流程

无（纯类型定义）。

设计要点：

- **终态判据不在类型层**：`utils/submission.isTerminalStatus` 是唯一判据（Pending/Compiling/Running 非终态，其余含 `Unknown` 全部终态）；类型全集与判据的对应关系有穷尽映射测试锁定。
- **失败原因随轮询结果一起回来**：`JudgementResult.errorMessage` 让「CE 编译错误是什么」在轮询通道就能到达选手，不必等点进详情页（`SubmissionDetail.errorMessage` 是同一信息的一次性查询口径）。轮询是选手感知评测失败的**唯一自动通道**。
- **状态码是 HOJ 原始码、含负数**：类型层的值域不是「0 起顺排」，任何此类假设都会把 UI 筛选/映射指向错误状态（`utils/submission.STATUS_OPTIONS` 曾因此整表偏移，见其文档的事故记录）。
- 单位约定：`timeMs`(ms) / `memoryKb`(KB) 后缀显式标单位；`submitTime`/`createdAt` 等 i64 时间戳一律 **epoch 秒**（Rust `parse_time` 口径），前端渲染时 ×1000。
- `score` / `oiRankScore` 仅 OI 赛制有意义（ACM 为 null/0）。
