# submission.service（提交服务）

> 源文件：`src/services/submission.service.ts`

## 职责

提交领域的服务层：代码提交与评测结果查询的转发，是 submissionStore 访问提交后端的唯一入口。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `SubmissionService.submitCode` | `(contestId, problemId, language, sourceCode) => Promise<string>` | 提交代码，返回 submissionId |
| `SubmissionService.pollJudgement` | `(submissionId) => Promise<JudgementResult>` | 查询一次评测结果（轮询节奏由 submissionStore 编排） |
| `submissionService` | 单例 | 全局唯一实例 |

## 直接依赖

- `@/bridge/submission.bridge`（`submitCode` / `getJudgement`）
- `@/types/submission`（仅 `JudgementResult` 类型）

## 被依赖

- `stores/submissionStore.ts` — 唯一调用方

## 逻辑流程

```
submitCode → bridge.submitCode → submissionId
pollJudgement → bridge.getJudgement → { status, score, timeMs, memoryKb }
```

设计要点：

- 当前为一比一转发，无额外编排——轮询间隔/终态判据/超时兜底全部在 store 层
  （节奏是状态机的一部分），服务层保持无状态。
- 命名为 `pollJudgement` 而非 `getJudgement`：向调用方明示「本方法供轮询场景使用，
  单次调用无轮询语义」。
