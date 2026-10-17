import { describe, expect, it } from 'vitest'
import { getClockSegments, getContestClock } from '@/utils/contest-clock'

describe('getContestClock', () => {
  it('counts down to the scheduled start and marks the final 24 hours as urgent', () => {
    expect(getContestClock('upcoming', 100_000, 200_000, 100_000 - 86_401)).toMatchObject({
      kind: 'countdown',
      seconds: 86_401,
      urgent: false,
    })
    expect(getContestClock('upcoming', 100_000, 200_000, 100_000 - 86_400)).toMatchObject({
      kind: 'countdown',
      seconds: 86_400,
      urgent: false,
    })
    expect(getContestClock('upcoming', 100_000, 200_000, 100_000 - 86_399)).toMatchObject({
      kind: 'countdown',
      seconds: 86_399,
      urgent: true,
    })
  })

  it('shows elapsed contest time and caps it at the scheduled duration', () => {
    expect(getContestClock('running', 1_000, 4_600, 2_234)).toMatchObject({
      kind: 'elapsed',
      seconds: 1_234,
      urgent: false,
    })
    expect(getContestClock('running', 1_000, 4_600, 9_000).seconds).toBe(3_600)
  })

  it('distinguishes an ended contest and unavailable schedule data', () => {
    expect(getContestClock('ended', 1_000, 2_000, 2_000).kind).toBe('ended')
    expect(getContestClock('none', null, null, 1_000).kind).toBe('unavailable')
  })
})

describe('getClockSegments', () => {
  it('formats fixed-width day, hour, minute and second segments', () => {
    expect(getClockSegments(1 * 86_400 + 2 * 3_600 + 3 * 60 + 4).map(({ value }) => value))
      .toEqual(['01', '02', '03', '04'])
  })

  it('uses stable placeholders while contest information is unavailable', () => {
    expect(getClockSegments(null).map(({ value }) => value)).toEqual(['--', '--', '--', '--'])
  })
})
