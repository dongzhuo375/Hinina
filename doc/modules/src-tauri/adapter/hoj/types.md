# types

## 职责
HOJ API DTO 类型定义 + 状态码映射。

## 核心类型
- `ApiResponse<T>` — 统一响应包装 `{status, msg, data}`
- `PageResult<T>` — 分页 `{records, total, size, current}`
- `LoginRequest` / `UserInfoVO` — 认证
- `ContestVO` / `ContestProblemVO` — 比赛
- `ProblemVO` / `ProblemInfoVO` / `TagVO` — 题目
- `JudgeVO` / `SubmissionInfoVO` / `SubmissionDetail` — 提交评测
- `map_status(i32)` → JudgementStatus
- `is_terminal_status(i32)` → bool
