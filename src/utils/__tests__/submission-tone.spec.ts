import { describe, expect, it } from 'vitest'
import { STATUS_TONE_CLASSES, statusToneClasses } from '@/utils/submission-tone'
import type { StatusTone } from '@/utils/submission'

const EXPECTED: Record<StatusTone, string> = {
  ac: 'border-[var(--color-ac-border)] bg-[var(--color-ac-bg)] text-[var(--color-success)]',
  wa: 'border-[var(--color-wa-border)] bg-[var(--color-wa-bg)] text-[var(--color-error)]',
  tle: 'border-[var(--color-pending-border)] bg-[var(--color-pending-bg)] text-[var(--color-warning)]',
  pending: 'border-[var(--color-pending-border)] bg-[var(--color-pending-bg)] text-[var(--color-info)]',
  system: 'border-[var(--border-color)] bg-[var(--bg-sidebar)] text-[var(--text-secondary)]',
  neutral: 'border-[var(--border-color)] bg-[var(--bg-sidebar)] text-[var(--text-secondary)]',
}

describe('statusToneClasses', () => {
  it.each(Object.entries(EXPECTED))('tone %s 使用共享完整类串', (tone, expected) => {
    expect(statusToneClasses(tone as StatusTone)).toBe(expected)
  })

  it('system 与 neutral 有意共用灰色样式', () => {
    expect(STATUS_TONE_CLASSES.system).toBe(STATUS_TONE_CLASSES.neutral)
  })

  it('未知运行时 tone 回退 neutral', () => {
    expect(statusToneClasses('unexpected' as StatusTone)).toBe(EXPECTED.neutral)
  })

  it('QuickSubmit 可仅覆盖 pending，保留其原有琥珀色', () => {
    const pending = 'border-[var(--color-pending-border)] bg-[var(--color-pending-bg)] text-[var(--color-warning)]'
    expect(statusToneClasses('pending', { pending })).toBe(pending)
    expect(statusToneClasses('ac', { pending })).toBe(EXPECTED.ac)
  })
})
