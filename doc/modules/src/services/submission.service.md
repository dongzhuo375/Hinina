# submission.service（提交服务）

> 源文件：`src/services/submission.service.ts`

## 职责

提交领域的服务层：代码提交、评测结果查询、提交历史与详情查询的转发，是前端访问提交后端的唯一入口。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `SubmissionService.submitCode` | `(contestId, problemId, language, sourceCode) => Promise<string>` | 提交代码，返回 submissionId |
| `SubmissionService.pollJudgement` | `(submissionId) => Promise<JudgementResult>` | 查询一次评测结果（轮询节奏由 submissionStore 编排） |
| `SubmissionService.listContestSubmissions` | `(query: SubmissionListQuery) => Promise<SubmissionPage>` | 本人提交历史（分页；onlyMine 后端强制） |
| `SubmissionService.getSubmissionDetail` | `(submissionId) => Promise<SubmissionDetail>` | 提交详情（含源代码 / CE 错误信息） |
| `SubmissionService.getSubmissionCases` | `(submissionId) => Promise<SubmissionCases>` | 测试点详情（含子任务分组） |
| `submissionService` | 单例 | 全局唯一实例 |

## 直接依赖

- `@/bridge/submission.bridge`（全部五个 IPC 函数）
- `@/types/submission`（仅类型）

## 被依赖

- `stores/submissionStore.ts` — 提交/轮询/历史
- `views/SubmissionDetailView.vue`、`components/editor/EditorConsoleBar.vue` — 详情与测试点直查（一次性查询，无需进 store）

## 逻辑流程

```
submitCode → bridge.submitCode → submissionId
pollJudgement → bridge.getJudgement → { status, score, timeMs, memoryKb }
listContestSubmissions → bridge → SubmissionPage
getSubmissionDetail / getSubmissionCases → bridge → 详情 / 测试点
```

设计要点：

- 一比一转发，无额外编排——轮询间隔/终态判据/超时兜底全部在 store 层
  （节奏是状态机的一部分），服务层保持无状态。
- 命名为 `pollJudgement` 而非 `getJudgement`：向调用方明示「本方法供轮询场景使用，
  单次调用无轮询语义」。
