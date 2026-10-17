import { describe, expect, it } from 'vitest'
import { briefRetryDelayMs } from '@/utils/brief-retry'

describe('briefRetryDelayMs', () => {
  it('uses three bounded exponential retry intervals', () => {
    expect([0, 1, 2].map(briefRetryDelayMs)).toEqual([2_000, 4_000, 8_000])
    expect(briefRetryDelayMs(3)).toBeNull()
  })

  it('rejects invalid retry indexes', () => {
    expect(briefRetryDelayMs(-1)).toBeNull()
    expect(briefRetryDelayMs(1.5)).toBeNull()
  })
})
