import type { JudgementStatus } from '@/types/submission'

/**
 * 非终态状态集：评测仍在排队/编译/运行，需要继续轮询。
 *
 * 判据与 Rust `adapter::hoj::types::is_terminal_status`（仅 0=Pending、1=Judging 为非终态）
 * 保持一致 —— 其余状态均为终态，包括因 HOJ 无对应枚举而映射为 `Unknown` 的
 * OLE/SE/RJE/FREQ/UE。若把 `Unknown` 当作非终态，这些提交将被无限轮询。
 */
const NON_TERMINAL_STATUSES: ReadonlySet<JudgementStatus> = new Set<JudgementStatus>([
  'Pending',
  'Compiling',
  'Running',
])

/** 评测是否已到达终态（停止轮询的判据） */
export function isTerminalStatus(status: JudgementStatus): boolean {
  return !NON_TERMINAL_STATUSES.has(status)
}
