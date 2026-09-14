# submission（提交与评测跨端契约类型）

> 源文件：`src/types/submission.ts`

## 职责

评测状态枚举与评测结果的跨端契约类型，与 Rust `core::entity::submission::{JudgementStatus, JudgementResult}` 对齐。

## 核心类型/函数

| 名称 | 形状 | 关键语义 |
|------|------|----------|
| `JudgementStatus` | 10 值字符串联合：`Pending / Compiling / Running / Accepted / WrongAnswer / TimeLimitExceeded / MemoryLimitExceeded / RuntimeError / CompilationError / Unknown` | OJ 无关的归一状态。HOJ 的 16 个状态码由 Rust `map_status` 归并（PE/SF→WrongAnswer，OLE/SE/RJE/FREQ/UE→Unknown，PA→Accepted 保守映射） |
| `JudgementResult` | `{ status, score, timeMs, memoryKb }` | 单次评测查询结果；**`timeMs` 毫秒 / `memoryKb` KB**（HOJ 提交详情的 time 为 ms、memory 为 KB，Rust 侧原样透传）；非终态时 status 为 Running 系、数值字段无意义 |

## 直接依赖

无（纯类型声明文件）

## 被依赖

- `bridge/submission.bridge.ts`、`services/submission.service.ts`、`stores/submissionStore.ts`、`utils/submission.ts`、`components/editor/EditorConsoleBar.vue`（均仅类型引用）

## 逻辑流程

无（纯类型定义）。

设计要点：

- **终态判据不在类型层**：`utils/submission.isTerminalStatus` 是唯一判据
  （Pending/Compiling/Running 为非终态，**Unknown 必须视为终态**——否则被归并进
  Unknown 的 OLE/SE/RJE 等会被无限轮询）；类型全集与判据的对应关系有测试锁定。
- 单位注意：`timeMs`(ms) 与 `memoryKb`(KB) 后缀显式标单位；submissionStore 回填到
  `SubmissionEntry.time/memory` 时保持原单位，展示层（EditorConsoleBar）按 ms 渲染。
- `score` 仅 OI 赛制有意义（ACM 恒 0），当前 UI 未消费。
