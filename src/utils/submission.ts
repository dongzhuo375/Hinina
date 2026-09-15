import type { JudgementStatus } from '@/types/submission'

/**
 * 非终态状态集：评测仍在排队/编译/运行，需要继续轮询。
 *
 * 判据与 Rust `adapter::hoj::types::is_terminal_status`（仅 0=Pending、1=Judging 为非终态）
 * 保持一致 —— 其余状态（含扩展后的 PE/OLE/SE/RJE/SF/PA/FREQ/UE）均为终态。
 * 若把 `Unknown` 当作非终态，无法识别的状态码将被无限轮询。
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

/**
 * 状态语义色调，驱动 UI pill 配色（组件层把 tone 映射到具体样式类）。
 *
 * 与设计系统 OJ 语义色对齐：ac=绿 / wa=红 / tle=琥珀 / pending=青（评测中）/
 * system=石板灰（系统类故障，非选手代码问题）/ neutral=灰（未知）。
 */
export type StatusTone = 'ac' | 'wa' | 'tle' | 'pending' | 'system' | 'neutral'

interface StatusMeta {
  /// 状态原词（HOJ 文案，架构约束「状态文案以接口返回为准」）
  label: string
  /// 缩写（表格 pill / 最新记录等紧凑场景）
  abbr: string
  tone: StatusTone
}

/// 变体 → 文案/缩写/色调 的唯一映射表（新增状态只改这里）
const STATUS_META: Readonly<Record<JudgementStatus, StatusMeta>> = {
  Pending: { label: 'Pending', abbr: 'PD', tone: 'pending' },
  Compiling: { label: 'Judging', abbr: 'JDG', tone: 'pending' },
  Running: { label: 'Judging', abbr: 'JDG', tone: 'pending' },
  Accepted: { label: 'Accepted', abbr: 'AC', tone: 'ac' },
  PartiallyAccepted: { label: 'Partially Accepted', abbr: 'PA', tone: 'ac' },
  WrongAnswer: { label: 'Wrong Answer', abbr: 'WA', tone: 'wa' },
  PresentationError: { label: 'Presentation Error', abbr: 'PE', tone: 'wa' },
  CompilationError: { label: 'Compile Error', abbr: 'CE', tone: 'wa' },
  RuntimeError: { label: 'Runtime Error', abbr: 'RE', tone: 'wa' },
  TimeLimitExceeded: { label: 'Time Limit Exceeded', abbr: 'TLE', tone: 'tle' },
  MemoryLimitExceeded: { label: 'Memory Limit Exceeded', abbr: 'MLE', tone: 'tle' },
  OutputLimitExceeded: { label: 'Output Limit Exceeded', abbr: 'OLE', tone: 'tle' },
  FrequentLimit: { label: 'Submit Frequent Limit Exceeded', abbr: 'FREQ', tone: 'tle' },
  SystemError: { label: 'System Error', abbr: 'SE', tone: 'system' },
  RemoteJudgeError: { label: 'Remote Judge Error', abbr: 'RJE', tone: 'system' },
  SubmitFailed: { label: 'Submitted Failed', abbr: 'SF', tone: 'system' },
  UnknownError: { label: 'Unknown Error', abbr: 'UE', tone: 'system' },
  Unknown: { label: 'Unknown', abbr: '?', tone: 'neutral' },
}

/** 状态原词（未收录的变体兜底为变体名本身，避免升级期前端崩溃） */
export function statusLabel(status: JudgementStatus): string {
  return STATUS_META[status]?.label ?? status
}

/** 状态缩写（紧凑 pill 用） */
export function statusAbbr(status: JudgementStatus): string {
  return STATUS_META[status]?.abbr ?? '?'
}

/** 状态语义色调 */
export function statusTone(status: JudgementStatus): StatusTone {
  return STATUS_META[status]?.tone ?? 'neutral'
}

/** 评测是否仍在进行中（非终态） */
export function isJudging(status: JudgementStatus): boolean {
  return !isTerminalStatus(status)
}

/**
 * 评测页状态筛选下拉选项（value = HOJ 状态码，与后端 `status` 查询参数对齐）。
 *
 * 只列赛场高频状态；冷门状态（OLE/RJE/SF/FREQ/UE）归入「全部状态」查看，
 * 避免下拉过长。选项顺序按选手关注度排列。
 */
export const STATUS_OPTIONS: readonly { value: number; label: string }[] = [
  { value: 5, label: 'Accepted' },
  { value: 4, label: 'Wrong Answer' },
  { value: 6, label: 'Time Limit Exceeded' },
  { value: 7, label: 'Memory Limit Exceeded' },
  { value: 9, label: 'Runtime Error' },
  { value: 2, label: 'Compile Error' },
  { value: 3, label: 'Presentation Error' },
  { value: 13, label: 'Partially Accepted' },
  { value: 0, label: 'Pending' },
  { value: 1, label: 'Judging' },
]
