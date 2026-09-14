# submission.bridge（提交 IPC 桥接）

> 源文件：`src/bridge/submission.bridge.ts`

## 职责

提交与评测相关 Tauri IPC 的薄封装：`submit_code` / `get_judgement` 两个 Command 的参数透传。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `submitCode` | `(contestId, problemId, language, sourceCode) => Promise<string>` | invoke `submit_code`，返回 submissionId（**参数含源代码**，日志安全由 `ipcInvoke` 的「不记 args」约束保障） |
| `getJudgement` | `(submissionId) => Promise<JudgementResult>` | invoke `get_judgement`，返回 `{ status, score, timeMs, memoryKb }`；非终态时 status 为 Running 系（轮询由 store 编排） |

## 直接依赖

- `@/bridge`（`ipcInvoke`）
- `@/types/submission`（仅 `JudgementResult` 类型）

## 被依赖

- `services/submission.service.ts` — 唯一调用方

## 逻辑流程

```
submission.service.submitCode / pollJudgement
  → ipcInvoke('submit_code' | 'get_judgement', …)
  → Rust commands::submission_cmd → SubmissionService → SubmissionProvider
```
