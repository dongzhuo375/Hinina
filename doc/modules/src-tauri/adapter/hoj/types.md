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

## 序列化约定
- 所有响应 DTO 使用 `#[serde(rename_all = "camelCase")]`，将 Rust snake_case 字段映射到 HOJ 的 camelCase JSON（如 `start_time` ↔ `startTime`、`submit_id` ↔ `submitId`、`role_list` ↔ `roleList`）。
- 服务端可能返回 `null` 的字段（如 `nickname`、`avatar`、`description`、`examples`）声明为 `Option<String>` 而非 `String`，避免 `null` 反序列化崩溃。
