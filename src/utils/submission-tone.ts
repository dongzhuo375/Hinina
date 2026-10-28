import type { StatusTone } from '@/utils/submission'

/// 提交状态 pill 的默认主题类；组件的语义色映射仍由 utils/submission.statusTone 决定
export const STATUS_TONE_CLASSES: Readonly<Record<StatusTone, string>> = {
  ac: 'border-[var(--color-ac-border)] bg-[var(--color-ac-bg)] text-[var(--color-success)]',
  wa: 'border-[var(--color-wa-border)] bg-[var(--color-wa-bg)] text-[var(--color-error)]',
  tle: 'border-[var(--color-pending-border)] bg-[var(--color-pending-bg)] text-[var(--color-warning)]',
  pending: 'border-[var(--color-pending-border)] bg-[var(--color-pending-bg)] text-[var(--color-info)]',
  system: 'border-[var(--border-color)] bg-[var(--bg-sidebar)] text-[var(--text-secondary)]',
  neutral: 'border-[var(--border-color)] bg-[var(--bg-sidebar)] text-[var(--text-secondary)]',
}

/// overrides 只用于确有历史视觉差异的消费方；未知 tone 仍回退 neutral
export function statusToneClasses(
  tone: StatusTone,
  overrides: Partial<Record<StatusTone, string>> = {},
): string {
  return overrides[tone] ?? STATUS_TONE_CLASSES[tone] ?? STATUS_TONE_CLASSES.neutral
}
