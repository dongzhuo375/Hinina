/// 评测状态，对应 Rust `core::entity::submission::JudgementStatus`。
export type JudgementStatus =
  | 'Pending'
  | 'Compiling'
  | 'Running'
  | 'Accepted'
  | 'WrongAnswer'
  | 'TimeLimitExceeded'
  | 'MemoryLimitExceeded'
  | 'RuntimeError'
  | 'CompilationError'
  | 'Unknown'

/// 评测结果详情，对应 Rust `core::entity::submission::JudgementResult`。
export interface JudgementResult {
  status: JudgementStatus
  score: number
  timeMs: number
  memoryKb: number
}
