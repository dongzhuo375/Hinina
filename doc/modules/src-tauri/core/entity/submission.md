# submission

## 职责
定义提交实体 `Submission`、评测状态枚举 `JudgementStatus` 及评测结果 `JudgementResult`，覆盖从 Pending 到 Acception/Error 的完整评测生命周期。

## 核心类型/函数
- **`Submission`** — 提交 struct，字段：`id`, `problem_id`, `language`, `source_code`, `status`
- **`JudgementStatus`** — 评测状态枚举：`Pending`, `Compiling`, `Running`, `Accepted`, `WrongAnswer`, `TimeLimitExceeded`, `MemoryLimitExceeded`, `RuntimeError`, `CompilationError`, `Unknown`
- **`JudgementResult`** — 评测结果 struct，字段：`status`, `score`, `time_ms`, `memory_kb`

## 直接依赖
- `serde::{Deserialize, Serialize}`

## 被依赖
- `core::event::app_event`（SubmissionEvent::Judged 携带 JudgementResult）
- `core::provider::submission`（SubmissionProvider trait 使用 Submission 和 JudgementResult）
- `commands::submission_cmd`

## 逻辑流程
无（纯类型定义）。
