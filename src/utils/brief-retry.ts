export const BRIEF_RETRY_DELAYS_MS = [2_000, 4_000, 8_000] as const

/** Return the delay for a retry after the given number of retries, or null when exhausted. */
export function briefRetryDelayMs(retriesAlreadyUsed: number): number | null {
  if (!Number.isInteger(retriesAlreadyUsed) || retriesAlreadyUsed < 0) return null
  return BRIEF_RETRY_DELAYS_MS[retriesAlreadyUsed] ?? null
}
