# submission

## 职责
定义提交与评测领域实体：评测状态枚举 `JudgementStatus`、轮询用评测结果 `JudgementResult`、提交列表条目 `SubmissionRecord` 与分页 `SubmissionPage`、查询参数 `SubmissionQuery`、提交详情 `SubmissionDetail`、测试点结果 `JudgeCase` / `SubTaskCases` / `SubmissionCases`。覆盖「提交 → 轮询 → 列表 → 详情 → 测试点」完整链路。除 `SubmissionQuery`（内部类型，不跨 IPC）外全部 camelCase 序列化。

## 核心类型/函数
- **`JudgementStatus`** — 评测状态枚举：`Pending`, `Compiling`, `Running`, `Accepted`, `WrongAnswer`, `TimeLimitExceeded`, `MemoryLimitExceeded`, `RuntimeError`, `CompilationError`, `PresentationError`, `OutputLimitExceeded`, `SystemError`, `RemoteJudgeError`, `SubmitFailed`, `PartiallyAccepted`, `FrequentLimit`, `UnknownError`, `Unknown`。**变体名即 IPC 序列化值**（前端按这些确切名称做文案与配色映射），完整覆盖 HOJ 全部状态码 0–15（映射表见 `adapter/hoj/types.rs` 的 `map_status`）；`PartiallyAccepted` 为独立变体，不再折算为 `Accepted`（P41 修复）
  - `fn is_terminal(&self) -> bool` — **核心层终态判据**：非终态仅 `Pending` / `Compiling` / `Running`，其余（含 `Unknown` 与系统类错误）一律终态。**四处判据必须同步**：本方法、`adapter::hoj::types::is_terminal_status`（HOJ 原始状态码 → 终态）、`adapter::hydro::types::is_terminal_status`（Hydro 原始状态码 → 终态；**22 FETCHED 特意折入 `Pending`** 而非 `Unknown`，否则轮询会在评测开始前停住并把在途结果写进终态缓存）、前端 `utils/submission.isTerminalStatus`。用途：提交详情/测试点**只有终态结果才可缓存**（评测中的结果随时会变）
- **`JudgementResult`** — 轮询用评测结果：`status`, `score: f64`, `time_ms: u64`, `memory_kb: u64`
- **`SubmissionRecord`** — 提交列表条目（比赛提交记录页用）：`submit_id` / `pid`（题目真实 ID）/ `display_pid`（如 "HOJ-1061"）/ `title` / `display_id`（比赛中序号如 "A"）均为 `String`；`username`, `submit_time: i64`（UTC 秒级时间戳）, `status`, `time_ms`, `memory_kb`, `score: Option<f64>`（OI 题得分，ACM 题为 None）, `length: u64`（代码字节数）, `language`
- **`SubmissionPage`** — 提交列表分页：`records: Vec<SubmissionRecord>`, `total`, `size`, `current`, `pages`
- **`SubmissionQuery`** — 提交列表查询参数（**内部类型，仅 `Debug + Clone`，不跨 IPC 序列化**）：`contest_id`, `current_page`, `limit`, `only_mine: bool`（产品决策：后端强制为 true，见 `commands/submission_cmd.rs`）, `problem_display_id: Option<String>`（按题目展示 ID 筛选）, `status: Option<i32>`（按 HOJ 状态码筛选）
- **`SubmissionDetail`** — 提交详情（含源代码与错误信息）：在列表字段基础上增加 `code: String`（HOJ 返回 null 时 Adapter 回退空串）, `error_message: Option<String>`（CE 时非空）, `judger: Option<String>`（判题机标识）, `oi_rank_score: Option<i32>`（OI 榜单计入分数）
- **`JudgeCase`** — 单个测试点结果：`case_id: i64`, `seq: i64`（测试点序号）, `status`, `time_ms`, `memory_kb`, `score: Option<f64>`, `group_num: Option<i64>`（子任务分组号，非子任务题为 None）
- **`SubTaskCases`** — 子任务分组（subtask 模式）：`group_num: i64`, `cases: Vec<JudgeCase>`
- **`SubmissionCases`** — 提交的全部测试点结果：`cases`（默认模式列表）, `sub_tasks`（子任务模式分组列表）, `mode: String`（判题模式："default" / "subtask_lowest" / "subtask_lowest_all" / "ergodic_without_skipped" 等）

## 直接依赖
- `serde::{Deserialize, Serialize}`

## 被依赖
- `core::event::app_event`（SubmissionEvent::Judged 携带 JudgementResult）
- `core::provider::submission`（SubmissionProvider trait 使用 JudgementResult / SubmissionPage / SubmissionQuery / SubmissionDetail / SubmissionCases）
- `adapter::hoj`（`map_status` 产出 JudgementStatus；`into_submission_record` / `into_submission_detail` / `into_judge_case` 映射为领域实体）
- `service::submission`（轮询与三个查询方法）
- `commands::submission_cmd`（提交列表 / 详情 / 测试点 Command 返回值）

## 逻辑流程
无（纯类型定义，`JudgementStatus::is_terminal` 为纯函数判据）。

## 测试
`src-tauri/src/core/entity/tests/submission_tests.rs`（由 `submission.rs` 底部 `#[cfg(test)] #[path = "tests/submission_tests.rs"] mod tests;` 引用）以**穷尽表**锁定全部 18 个变体的终态性（新增状态时表必须同步，否则非终态集合漂移会让评测中的提交被缓存、界面停在「评测中」），并单独断言非终态集合恰好是 `Pending` / `Compiling` / `Running`。

> 历史说明：旧版的 `Submission` struct（`id/problem_id/language/source_code/status`）从未被任何调用方使用，属死代码，已随本次提交列表/详情能力重写一并删除。
