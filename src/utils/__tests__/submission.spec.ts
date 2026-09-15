import { describe, expect, it } from 'vitest'
import { isTerminalStatus } from '@/utils/submission'
import type { JudgementStatus } from '@/types/submission'

/// 非终态：评测仍在排队/编译/运行
const NON_TERMINAL: JudgementStatus[] = ['Pending', 'Compiling', 'Running']

/// 终态：与 Rust `adapter::hoj::types::is_terminal_status`（仅 HOJ 状态码 0/1 为非终态）对齐
const TERMINAL: JudgementStatus[] = [
  'Accepted',
  'WrongAnswer',
  'TimeLimitExceeded',
  'MemoryLimitExceeded',
  'RuntimeError',
  'CompilationError',
  'PresentationError',
  'OutputLimitExceeded',
  'SystemError',
  'RemoteJudgeError',
  'SubmitFailed',
  'PartiallyAccepted',
  'FrequentLimit',
  'UnknownError',
  'Unknown',
]

/// 编译期穷尽性检查：`Record<JudgementStatus, boolean>` 要求列出全部取值，
/// 将来给 JudgementStatus 新增状态时此处会直接类型报错，逼迫显式归类，
/// 而不是像从前那样漏掉一个状态（Unknown 无限轮询 bug 的成因）。
const TERMINAL_MAP: Record<JudgementStatus, boolean> = {
  Pending: false,
  Compiling: false,
  Running: false,
  Accepted: true,
  WrongAnswer: true,
  TimeLimitExceeded: true,
  MemoryLimitExceeded: true,
  RuntimeError: true,
  CompilationError: true,
  PresentationError: true,
  OutputLimitExceeded: true,
  SystemError: true,
  RemoteJudgeError: true,
  SubmitFailed: true,
  PartiallyAccepted: true,
  FrequentLimit: true,
  UnknownError: true,
  Unknown: true,
}

describe('isTerminalStatus', () => {
  it.each(NON_TERMINAL)('%s 为非终态，应继续轮询', (status) => {
    expect(isTerminalStatus(status)).toBe(false)
  })

  it.each(TERMINAL)('%s 为终态，应停止轮询', (status) => {
    expect(isTerminalStatus(status)).toBe(true)
  })

  it('回归：Unknown 必须视为终态', () => {
    // 无法识别的状态码被 Rust map_status 归为 Unknown。
    // 此前 View 内硬编码的终态列表漏掉 Unknown，导致这些提交被无限轮询。
    expect(isTerminalStatus('Unknown')).toBe(true)
  })

  it('终态清单与 JudgementStatus 全量取值一一对应', () => {
    const entries = Object.entries(TERMINAL_MAP)
    expect(entries).toHaveLength(18)
    for (const [status, terminal] of entries) {
      expect(isTerminalStatus(status as JudgementStatus), status).toBe(terminal)
    }
    // 两份清单与穷尽映射表必须一致，防止其中一处被单独改动
    expect(entries.filter(([, t]) => !t).map(([s]) => s)).toEqual(NON_TERMINAL)
    expect(entries.filter(([, t]) => t).map(([s]) => s)).toEqual(TERMINAL)
  })
})
