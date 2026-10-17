import type { ContestPhase } from '@/utils/contest'

export interface ContestClock {
  kind: 'countdown' | 'elapsed' | 'ended' | 'unavailable'
  seconds: number | null
  urgent: boolean
}

export interface ClockSegment {
  key: 'days' | 'hours' | 'minutes' | 'seconds'
  label: string
  value: string
}

const DAY_SECONDS = 24 * 60 * 60

/** Build the login-page clock from the same phase/time contract used by the contest shell. */
export function getContestClock(
  phase: ContestPhase,
  startTime: number | null,
  endTime: number | null,
  now: number,
): ContestClock {
  if (phase === 'upcoming' && startTime !== null) {
    const seconds = Math.max(0, startTime - now)
    return { kind: 'countdown', seconds, urgent: seconds < DAY_SECONDS }
  }

  if (phase === 'running' && startTime !== null && endTime !== null) {
    const duration = Math.max(0, endTime - startTime)
    const seconds = Math.min(duration, Math.max(0, now - startTime))
    return { kind: 'elapsed', seconds, urgent: false }
  }

  if (phase === 'ended') return { kind: 'ended', seconds: null, urgent: false }
  return { kind: 'unavailable', seconds: null, urgent: false }
}

export function getClockSegments(seconds: number | null): ClockSegment[] {
  const values = seconds === null
    ? null
    : [
        Math.floor(seconds / DAY_SECONDS),
        Math.floor((seconds % DAY_SECONDS) / 3600),
        Math.floor((seconds % 3600) / 60),
        seconds % 60,
      ]

  return [
    { key: 'days', label: '天', value: values ? String(values[0]).padStart(2, '0') : '--' },
    { key: 'hours', label: '时', value: values ? String(values[1]).padStart(2, '0') : '--' },
    { key: 'minutes', label: '分', value: values ? String(values[2]).padStart(2, '0') : '--' },
    { key: 'seconds', label: '秒', value: values ? String(values[3]).padStart(2, '0') : '--' },
  ]
}
