import { describe, expect, it } from 'vitest'
import {
  effectiveLimits,
  formatLimitsSummary,
  formatMemoryLimit,
  formatTimeLimit,
  isDoubleLimitLanguage,
} from '@/utils/limits'
import type { ProblemLimits } from '@/types/rank'

/// HOJ 后台默认 limits（HOJ-Problem-Limits-API.md §4：timeLimit 默认 1000ms、memoryLimit 默认 256MB）
const DEFAULT_LIMITS: ProblemLimits = { displayId: 'A', timeLimit: 1000, memoryLimit: 256 }

describe('formatTimeLimit', () => {
  it('满 1 秒按秒显示，保留 1 位小数', () => {
    expect(formatTimeLimit(1000)).toBe('1.0s')
    expect(formatTimeLimit(2500)).toBe('2.5s')
    expect(formatTimeLimit(10000)).toBe('10.0s')
  })

  it('不足 1 秒按毫秒整数显示', () => {
    expect(formatTimeLimit(500)).toBe('500ms')
    expect(formatTimeLimit(999)).toBe('999ms')
    expect(formatTimeLimit(1)).toBe('1ms')
  })

  it('非法/0 返回 —', () => {
    expect(formatTimeLimit(0)).toBe('—')
    expect(formatTimeLimit(-100)).toBe('—')
    expect(formatTimeLimit(Number.NaN)).toBe('—')
    expect(formatTimeLimit(Number.POSITIVE_INFINITY)).toBe('—')
  })
})

describe('formatMemoryLimit', () => {
  it('不足 1 GB 按 MB 整数显示', () => {
    expect(formatMemoryLimit(256)).toBe('256 MB')
    expect(formatMemoryLimit(512)).toBe('512 MB')
    expect(formatMemoryLimit(1023)).toBe('1023 MB')
  })

  it('满 1024 MB 换算为 GB，去掉尾零', () => {
    expect(formatMemoryLimit(1024)).toBe('1 GB')
    expect(formatMemoryLimit(2048)).toBe('2 GB')
    expect(formatMemoryLimit(1536)).toBe('1.5 GB')
    expect(formatMemoryLimit(1280)).toBe('1.25 GB')
  })

  it('非法/0 返回 —', () => {
    expect(formatMemoryLimit(0)).toBe('—')
    expect(formatMemoryLimit(-1)).toBe('—')
    expect(formatMemoryLimit(Number.NaN)).toBe('—')
  })
})

describe('isDoubleLimitLanguage', () => {
  it('c/cpp 为 1 倍基准（HOJ 判据是源文件后缀 .c/.cpp，见文档 §5）', () => {
    expect(isDoubleLimitLanguage('c')).toBe(false)
    expect(isDoubleLimitLanguage('cpp')).toBe(false)
  })

  it('大小写与首尾空格不影响判定', () => {
    expect(isDoubleLimitLanguage('C')).toBe(false)
    expect(isDoubleLimitLanguage('CPP')).toBe(false)
    expect(isDoubleLimitLanguage(' cpp ')).toBe(false)
  })

  it('java/python 等其它语言时间与内存都 ×2', () => {
    expect(isDoubleLimitLanguage('java')).toBe(true)
    expect(isDoubleLimitLanguage('python')).toBe(true)
  })

  it('HOJ 显示名同样正确判定（语言权威值已是显示名）', () => {
    expect(isDoubleLimitLanguage('C')).toBe(false)
    expect(isDoubleLimitLanguage('C++')).toBe(false)
    expect(isDoubleLimitLanguage('C++17 (GCC 13.2)')).toBe(false)
    expect(isDoubleLimitLanguage('Java')).toBe(true)
    expect(isDoubleLimitLanguage('Python')).toBe(true)
    expect(isDoubleLimitLanguage('Go')).toBe(true)
  })

  it('未知/空语言 id 按保守放大处理（宁可显示宽松阈值）', () => {
    expect(isDoubleLimitLanguage('go')).toBe(true)
    expect(isDoubleLimitLanguage('')).toBe(true)
  })
})

describe('effectiveLimits', () => {
  it('1 倍语言返回等值 limits', () => {
    expect(effectiveLimits(DEFAULT_LIMITS, 'cpp')).toEqual(DEFAULT_LIMITS)
  })

  it('2 倍语言时间与内存都 ×2，displayId 不变', () => {
    expect(effectiveLimits(DEFAULT_LIMITS, 'java')).toEqual({
      displayId: 'A',
      timeLimit: 2000,
      memoryLimit: 512,
    })
  })

  it('纯函数：不修改入参，且返回新对象', () => {
    const result = effectiveLimits(DEFAULT_LIMITS, 'python')
    expect(DEFAULT_LIMITS).toEqual({ displayId: 'A', timeLimit: 1000, memoryLimit: 256 })
    expect(result).not.toBe(DEFAULT_LIMITS)
    // 1 倍语言同样返回副本，调用方可安全持有
    expect(effectiveLimits(DEFAULT_LIMITS, 'c')).not.toBe(DEFAULT_LIMITS)
  })
})

describe('formatLimitsSummary', () => {
  it('卡片紧凑文案：时间 / 内存', () => {
    expect(formatLimitsSummary(DEFAULT_LIMITS)).toBe('1.0s / 256 MB')
  })

  it('与 effectiveLimits 组合展示 2 倍语言的实际阈值', () => {
    expect(formatLimitsSummary(effectiveLimits(DEFAULT_LIMITS, 'java'))).toBe('2.0s / 512 MB')
  })

  it('非法 limits 渲染占位符而非空白', () => {
    expect(formatLimitsSummary({ displayId: 'B', timeLimit: 0, memoryLimit: 0 })).toBe('— / —')
  })
})
