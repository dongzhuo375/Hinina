import { describe, expect, it } from 'vitest'
import {
  findFirstFailedCase,
  formatClock,
  formatCodeLength,
  formatDurationHms,
  formatMemoryKb,
  formatMsToSeconds,
  isTerminalStatus,
  mapLanguageToMonaco,
} from '@/utils/submission'
import type { JudgeCase, JudgementStatus, SubmissionCases } from '@/types/submission'

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

describe('formatClock', () => {
  it('epoch 秒按本地时区格式化为 HH:MM:SS', () => {
    // 用本地时间构造 epoch，避免测试机时区影响断言
    const local = new Date(2024, 4, 1, 4, 32, 10)
    expect(formatClock(Math.floor(local.getTime() / 1000))).toBe('04:32:10')
  })

  it('个位数时分秒补零', () => {
    const local = new Date(2024, 0, 1, 1, 2, 3)
    expect(formatClock(Math.floor(local.getTime() / 1000))).toBe('01:02:03')
  })
})

describe('formatDurationHms', () => {
  it('正秒数格式化为 HH:MM:SS', () => {
    expect(formatDurationHms(1670)).toBe('00:27:50')
    expect(formatDurationHms(3661)).toBe('01:01:01')
    expect(formatDurationHms(0)).toBe('00:00:00')
  })

  it('负值（赛前提交）钳制为 --:--:--', () => {
    expect(formatDurationHms(-1)).toBe('--:--:--')
    expect(formatDurationHms(-3600)).toBe('--:--:--')
  })

  it('非有限值钳制为 --:--:--', () => {
    expect(formatDurationHms(Number.NaN)).toBe('--:--:--')
    expect(formatDurationHms(Number.POSITIVE_INFINITY)).toBe('--:--:--')
  })
})

describe('formatMemoryKb', () => {
  it('小于 1024 KB 原样显示 KB', () => {
    expect(formatMemoryKb(512)).toBe('512 KB')
    expect(formatMemoryKb(1023.4)).toBe('1023 KB')
  })

  it('不小于 1024 KB 换算为 MB（1 位小数）', () => {
    expect(formatMemoryKb(1024)).toBe('1.0 MB')
    expect(formatMemoryKb(2150)).toBe('2.1 MB')
    expect(formatMemoryKb(15155)).toBe('14.8 MB')
  })

  it('非正值显示 -（评测中/未回填）', () => {
    expect(formatMemoryKb(0)).toBe('-')
    expect(formatMemoryKb(-1)).toBe('-')
  })
})

describe('formatCodeLength', () => {
  it('字节换算为 KB（1 位小数）', () => {
    expect(formatCodeLength(1229)).toBe('1.2 KB')
    expect(formatCodeLength(410)).toBe('0.4 KB')
    expect(formatCodeLength(0)).toBe('0.0 KB')
  })

  it('非法值显示 -', () => {
    expect(formatCodeLength(-1)).toBe('-')
    expect(formatCodeLength(Number.NaN)).toBe('-')
  })
})

describe('formatMsToSeconds', () => {
  it('毫秒换算为秒（2 位小数）', () => {
    expect(formatMsToSeconds(2010)).toBe('2.01s')
    expect(formatMsToSeconds(1200)).toBe('1.20s')
    expect(formatMsToSeconds(16)).toBe('0.02s')
  })

  it('非法值显示 -', () => {
    expect(formatMsToSeconds(-5)).toBe('-')
    expect(formatMsToSeconds(Number.NaN)).toBe('-')
  })
})

describe('mapLanguageToMonaco', () => {
  it('HOJ 显示名归一到 Monaco language id', () => {
    expect(mapLanguageToMonaco('C++')).toBe('cpp')
    expect(mapLanguageToMonaco('C++17 (GCC 13.2)')).toBe('cpp')
    expect(mapLanguageToMonaco('cpp')).toBe('cpp')
    expect(mapLanguageToMonaco('C')).toBe('c')
    expect(mapLanguageToMonaco('C (GCC 13.2)')).toBe('c')
    expect(mapLanguageToMonaco('Java')).toBe('java')
    expect(mapLanguageToMonaco('Java 17 (OpenJDK)')).toBe('java')
    expect(mapLanguageToMonaco('Python 3.10')).toBe('python')
    expect(mapLanguageToMonaco('python')).toBe('python')
  })

  it('无法识别的语言回退 cpp', () => {
    expect(mapLanguageToMonaco('')).toBe('cpp')
    expect(mapLanguageToMonaco('Rust')).toBe('cpp')
  })
})

describe('findFirstFailedCase', () => {
  function makeCase(over: Partial<JudgeCase> = {}): JudgeCase {
    return {
      caseId: 1,
      seq: 1,
      status: 'Accepted',
      timeMs: 100,
      memoryKb: 1024,
      score: null,
      groupNum: null,
      ...over,
    }
  }

  function makeCases(over: Partial<SubmissionCases> = {}): SubmissionCases {
    return { cases: [], subTasks: [], mode: 'default', ...over }
  }

  it('平铺 cases 中返回首个非 Accepted 测试点', () => {
    const result = makeCases({
      cases: [
        makeCase({ caseId: 1, seq: 1 }),
        makeCase({ caseId: 2, seq: 2, status: 'WrongAnswer', timeMs: 2010 }),
        makeCase({ caseId: 3, seq: 3, status: 'TimeLimitExceeded' }),
      ],
    })
    const first = findFirstFailedCase(result)
    expect(first?.seq).toBe(2)
    expect(first?.status).toBe('WrongAnswer')
  })

  it('平铺 cases 全部通过时返回 null（不再查 subTasks）', () => {
    const result = makeCases({
      cases: [makeCase({ caseId: 1, seq: 1 })],
      subTasks: [
        { groupNum: 1, cases: [makeCase({ caseId: 9, seq: 9, status: 'WrongAnswer' })] },
      ],
    })
    expect(findFirstFailedCase(result)).toBeNull()
  })

  it('子任务制（cases 为空）按 groupNum、组内按 seq 展开查找', () => {
    const result = makeCases({
      mode: 'subtask',
      // 刻意乱序：helper 须按 groupNum / seq 排序后取首个非 AC
      subTasks: [
        {
          groupNum: 2,
          cases: [
            makeCase({ caseId: 3, seq: 2, status: 'TimeLimitExceeded', groupNum: 2 }),
            makeCase({ caseId: 2, seq: 1, groupNum: 2 }),
          ],
        },
        {
          groupNum: 1,
          cases: [
            makeCase({ caseId: 4, seq: 2, status: 'WrongAnswer', timeMs: 1500, groupNum: 1 }),
            makeCase({ caseId: 1, seq: 1, groupNum: 1 }),
          ],
        },
      ],
    })
    const first = findFirstFailedCase(result)
    expect(first?.caseId).toBe(4)
    expect(first?.seq).toBe(2)
  })

  it('cases 与 subTasks 均为空时返回 null', () => {
    expect(findFirstFailedCase(makeCases())).toBeNull()
  })
})
