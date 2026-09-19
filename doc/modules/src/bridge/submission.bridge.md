# submission.bridge（提交 IPC 桥接）

> 源文件：`src/bridge/submission.bridge.ts`

## 职责

提交与评测相关 Tauri IPC 的薄封装：`submit_code` / `get_judgement` / `list_contest_submissions` / `get_submission_detail` / `get_submission_cases` 五个 Command 的参数透传。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `submitCode` | `(contestId, problemId, displayId, language, sourceCode) => Promise<string>` | invoke `submit_code`，返回 submissionId（**参数含源代码**，日志安全由 `ipcInvoke` 的「不记 args」约束保障）。`problemId` 与 `displayId` 是同一道题的两个标识、必须都传：`problemId` 供工作区隔离/状态查询，`displayId` 供服务端定位比赛题目 —— **HOJ 的提交接口收的是展示题号**，传数字 pid 会让服务端查不到 `contest_problem` 而抛 NPE（HTTP 500，实测）；Hydro 收真实 ID，由 Adapter 各取所需 |
| `getJudgement` | `(submissionId) => Promise<JudgementResult>` | invoke `get_judgement`，返回 `{ status, score, timeMs, memoryKb, errorMessage }`；非终态时 status 为 Running 系（轮询由 store 编排） |
| `listContestSubmissions` | `(query: SubmissionListQuery) => Promise<SubmissionPage>` | invoke `list_contest_submissions`；「只看本人」由 Rust 命令层强制（onlyMine 恒 true），前端不传该参数 |
| `getSubmissionDetail` | `(submissionId) => Promise<SubmissionDetail>` | invoke `get_submission_detail`（含源代码与 CE 错误信息） |
| `getSubmissionCases` | `(submissionId) => Promise<SubmissionCases>` | invoke `get_submission_cases`（测试点明细，含子任务分组） |

## 直接依赖

- `@/bridge`（`ipcInvoke`）
- `@/types/submission`（仅类型）

## 被依赖

- `services/submission.service.ts` — 唯一调用方

## 逻辑流程

```
submission.service.*
  → ipcInvoke('submit_code' | 'get_judgement' | 'list_contest_submissions'
              | 'get_submission_detail' | 'get_submission_cases', …)
  → Rust commands::submission_cmd → SubmissionService → SubmissionProvider(HOJ)
```
