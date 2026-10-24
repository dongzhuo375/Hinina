import { describe, expect, it } from 'vitest'
import type {
  Hinina,
  JsonValue,
  PluginContest,
  PluginContestProblem,
  PluginEventEnvelope,
  PluginHttpRequest,
  PluginRankCell,
  PluginRankPage,
  PluginRankQuery,
  PluginRankRow,
  PluginStorageValue,
} from '../../../hinina'
import activate from '../index'

/**
 * Logic tests for the reference balloon plugin: the Hinina surface is mocked
 * (same layering rule as the frontend — tests mock the layer below), so
 * nothing touches a real host, OJ, or network.
 */

type ResyncEvent = { type: 'resync.required'; lost: number }
type ResyncEnvelope = PluginEventEnvelope<ResyncEvent>
type ResyncHandler = (event: ResyncEvent, envelope: ResyncEnvelope) => void | Promise<void>
type HttpOutcome = 'ok' | 'http500' | 'timeout' | 'context_expired'

interface Harness {
  hinina: Hinina
  /** `${level} ${message}` per written line. */
  logs: string[]
  /** Ordered capability calls (probe traffic excluded). */
  calls: string[]
  requests: PluginHttpRequest[]
  storage: Map<string, JsonValue>
  handlers: ResyncHandler[]
  approved: Set<string>
  contest: PluginContest | null
  problems: PluginContestProblem[]
  rankPages: PluginRankPage[]
  rankError: { code: string } | null
  contextMode: 'ok' | 'expired'
  httpPlan: HttpOutcome[]
}

const ALL_PERMISSIONS = [
  'events.subscribe',
  'contest.read',
  'contest.rank',
  'storage.read',
  'storage.write',
  'network.request',
]

/** Deep JSON equality, independent of key order (mirrors the host's serde Value equality). */
function stable(value: JsonValue): string {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return JSON.stringify(value) ?? 'null'
  const entries = Object.entries(value)
    .filter(([, v]) => v !== undefined)
    .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
  return `{${entries.map(([k, v]) => `${JSON.stringify(k)}:${stable(v)}`).join(',')}}`
}

function contestFixture(overrides: Partial<PluginContest> = {}): PluginContest {
  return {
    contestId: 'c1',
    title: 'Test Contest',
    startsAt: 1_000_000_000_000,
    endsAt: 1_000_003_600_000,
    contestType: 'acm',
    sealed: false,
    sealedSince: null,
    ...overrides,
  }
}

function rowFixture(overrides: Partial<PluginRankRow> = {}): PluginRankRow {
  return {
    rank: 1,
    teamKey: 'team-a',
    displayName: 'Team A',
    school: 'School A',
    ac: 1,
    totalScore: null,
    cells: {},
    ...overrides,
  }
}

function cellFixture(overrides: Partial<PluginRankCell> = {}): PluginRankCell {
  return {
    displayId: 'A',
    solved: true,
    firstAc: true,
    acTime: 100,
    errorNum: 0,
    tryNum: 1,
    score: null,
    afterContest: false,
    ...overrides,
  }
}

function pageFixture(rows: PluginRankRow[], hasMore = false): PluginRankPage {
  return { rows, page: 1, hasMore }
}

function createHarness(): Harness {
  const logs: string[] = []
  const calls: string[] = []
  const requests: PluginHttpRequest[] = []
  const storage = new Map<string, JsonValue>()
  const handlers: ResyncHandler[] = []
  const harness: Harness = {
    hinina: null as unknown as Hinina,
    logs,
    calls,
    requests,
    storage,
    handlers,
    approved: new Set(ALL_PERMISSIONS),
    contest: contestFixture(),
    problems: [
      { problemId: 'P1', displayId: 'A', title: 'Problem A', color: 'red' },
      { problemId: 'P2', displayId: 'B', title: 'Problem B', color: 'blue' },
      { problemId: 'P3', displayId: 'C', title: 'Problem C', color: '' },
    ],
    rankPages: [],
    rankError: null,
    contextMode: 'ok',
    httpPlan: [],
  }
  const frozen = (code: string): { code: string } => Object.freeze({ code })
  const permissionError = (permission: string) => Object.freeze({ code: 'permission_denied', permission })
  const stored = (key: string): PluginStorageValue =>
    storage.has(key) ? { state: 'value', value: storage.get(key) as JsonValue } : { state: 'missing' }

  const hinina = {
    events: {
      on: (_event: 'resync.required', handler: ResyncHandler) => {
        handlers.push(handler)
        return () => undefined
      },
    },
    context: {
      current: async () => {
        if (harness.contextMode === 'expired') throw frozen('context_expired')
        return 'ctx-1'
      },
    },
    contest: {
      current: async () => harness.contest,
      get: async () => null,
      problems: async () => {
        calls.push('problems')
        return harness.problems
      },
      rank: async (context: string, query: PluginRankQuery) => {
        if (!harness.approved.has('contest.rank')) throw permissionError('contest.rank')
        if (!harness.approved.has('contest.read')) throw permissionError('contest.read')
        // The activation probe uses a dummy context; it must fail the scope
        // check before any provider work, and stay invisible in `calls`.
        if (context.includes('probe')) throw frozen('context_expired')
        calls.push(`rank:${query.page}`)
        if (harness.rankError !== null) throw frozen(harness.rankError.code)
        const page = harness.rankPages.shift()
        if (page === undefined) throw frozen('limit_exceeded')
        return page
      },
    },
    submission: {
      get: async () => null,
      list: async () => {
        throw new Error('unused by the balloon plugin')
      },
    },
    identity: { current: async () => null },
    storage: {
      get: async (key: string) => stored(key),
      set: async (key: string, value: JsonValue) => {
        if (key === 'balloon.probe') return
        calls.push(`set:${key}`)
        storage.set(key, value)
      },
      delete: async (key: string) => {
        storage.delete(key)
      },
      compareAndSet: async (key: string, expected: PluginStorageValue, value: JsonValue) => {
        if (!harness.approved.has('storage.read')) throw permissionError('storage.read')
        if (!harness.approved.has('storage.write')) throw permissionError('storage.write')
        if (key === 'balloon.probe') return false
        calls.push(`cas:${key}`)
        if (stable(stored(key) as unknown as JsonValue) !== stable(expected as unknown as JsonValue)) return false
        storage.set(key, value)
        return true
      },
    },
    http: {
      request: async (context: string, request: PluginHttpRequest) => {
        if (!harness.approved.has('network.request')) throw permissionError('network.request')
        if (context.includes('probe')) throw frozen('context_expired')
        requests.push(request)
        calls.push('http')
        const outcome = harness.httpPlan.shift() ?? 'ok'
        if (outcome === 'http500') return { ok: false, status: 500, headers: {}, body: '' }
        if (outcome === 'timeout') throw frozen('timeout')
        if (outcome === 'context_expired') throw frozen('context_expired')
        return { ok: true, status: 200, headers: {}, body: '' }
      },
    },
    notification: {
      info: async () => undefined,
      success: async () => undefined,
      warn: async () => undefined,
      error: async () => undefined,
    },
    log: {
      debug: async (message: string) => {
        logs.push(`debug ${message}`)
      },
      info: async (message: string) => {
        logs.push(`info ${message}`)
      },
      warn: async (message: string) => {
        logs.push(`warn ${message}`)
      },
      error: async (message: string) => {
        logs.push(`error ${message}`)
      },
    },
  }
  harness.hinina = hinina as unknown as Hinina
  return harness
}

async function start(harness: Harness, endpoint = 'https://httpbin.org/post'): Promise<void> {
  await activate(harness.hinina, { endpoint })
}

async function resync(harness: Harness, now: number): Promise<void> {
  expect(harness.handlers.length).toBe(1)
  const handler = harness.handlers[0] as unknown as ResyncHandler
  await handler({ type: 'resync.required', lost: 0 }, {
    version: 2,
    sequence: 1,
    occurredAt: now,
    event: { type: 'resync.required', lost: 0 },
  })
}

function records(harness: Harness, contestId = 'c1'): Record<string, { state: string }> {
  const value = harness.storage.get(`balloon.records:${contestId}`)
  return (value ?? {}) as Record<string, { state: string }>
}

function bodies(harness: Harness): Array<Record<string, JsonValue>> {
  return harness.requests.map(request => request.body as Record<string, JsonValue>)
}

describe('balloon reference plugin activation', () => {
  it('registers the resync handler when every permission is approved', async () => {
    const harness = createHarness()
    await start(harness)
    expect(harness.handlers.length).toBe(1)
    // The probes are side-effect free: no rank page consumed, nothing written.
    expect(harness.calls).toEqual([])
    expect(harness.storage.has('balloon.probe')).toBe(false)
  })

  it('fails activation with the missing permission when contest.rank is not approved', async () => {
    const harness = createHarness()
    harness.approved.delete('contest.rank')
    await expect(start(harness)).rejects.toMatchObject({
      code: 'permission_denied',
      permission: 'contest.rank',
    })
    expect(harness.handlers.length).toBe(0)
  })

  it('rejects an options object without an https endpoint', async () => {
    const harness = createHarness()
    await expect(activate(harness.hinina, {})).rejects.toMatchObject({ code: 'invalid_input' })
    await expect(activate(harness.hinina, { endpoint: 'http://insecure.example' })).rejects.toMatchObject({
      code: 'invalid_input',
    })
  })
})

describe('balloon reference plugin rounds', () => {
  it('skips the whole round while sealed and never reads the board', async () => {
    const harness = createHarness()
    await start(harness)
    harness.contest = contestFixture({ sealed: true, sealedSince: 1_000_000_050_000 })
    harness.rankPages = [pageFixture([rowFixture({ cells: { A: cellFixture() } })])]
    await resync(harness, 1_000_000_060_000)
    // The only permitted activity is persisting the seal window itself.
    expect(harness.calls).toEqual(['set:balloon.seal:c1'])
    expect(harness.requests).toEqual([])
    expect(harness.logs.some(line => line.includes('sealed'))).toBe(true)
    expect(harness.storage.get('balloon.seal:c1')).toEqual({ since: 1_000_000_050_000, revealedAt: null })
  })

  it('quietly ends the round when no contest is selected', async () => {
    const harness = createHarness()
    await start(harness)
    harness.contest = null
    await resync(harness, 1_000_000_060_000)
    expect(harness.calls).toEqual([])
    expect(harness.logs.some(line => line.includes('no contest'))).toBe(true)
  })

  it('dedupes rows across pages by teamKey and skips starred teams', async () => {
    const harness = createHarness()
    await start(harness)
    harness.rankPages = [
      pageFixture(
        [
          rowFixture({ cells: { A: cellFixture() } }),
          rowFixture({ rank: -1, teamKey: 'team-star', cells: { B: cellFixture({ displayId: 'B' }) } }),
        ],
        true,
      ),
      pageFixture([
        // Same team again on a later page (live view drift): its cells are ignored.
        rowFixture({ rank: 2, cells: { C: cellFixture({ displayId: 'C' }) } }),
        rowFixture({ rank: 3, teamKey: 'team-b', displayName: 'Team B', cells: { B: cellFixture({ displayId: 'B' }) } }),
      ]),
    ]
    await resync(harness, 1_000_000_100_000)
    const keys = bodies(harness).map(body => body.idempotencyKey)
    expect(keys).toEqual(['c1:team-a:A', 'c1:team-b:B'])
    expect(Object.keys(records(harness)).sort()).toEqual(['team-a:A', 'team-b:B'])
  })

  it('claims pending before dispatching and marks sent only after a 2xx', async () => {
    const harness = createHarness()
    await start(harness)
    harness.rankPages = [pageFixture([rowFixture({ cells: { A: cellFixture() } })])]
    await resync(harness, 1_000_000_100_000)
    expect(harness.calls.indexOf('cas:balloon.records:c1')).toBeLessThan(harness.calls.indexOf('http'))
    expect(records(harness)['team-a:A']).toEqual(
      expect.objectContaining({ state: 'sent', displayId: 'A', color: 'red' }),
    )
    // A repeated round dispatches nothing: the record is known.
    harness.rankPages = [pageFixture([rowFixture({ cells: { A: cellFixture() } })])]
    await resync(harness, 1_000_000_160_000)
    expect(harness.requests.length).toBe(1)
    expect(harness.logs.some(line => line.includes('nothing to do'))).toBe(true)
  })

  it('keeps pending on 5xx and retries the same idempotency key next round', async () => {
    const harness = createHarness()
    await start(harness)
    harness.rankPages = [pageFixture([rowFixture({ cells: { A: cellFixture() } })])]
    harness.httpPlan = ['http500']
    await resync(harness, 1_000_000_100_000)
    expect(records(harness)['team-a:A']).toEqual(expect.objectContaining({ state: 'pending' }))
    expect(harness.logs.some(line => line.includes('HTTP 500'))).toBe(true)
    harness.rankPages = [pageFixture([rowFixture({ cells: { A: cellFixture() } })])]
    await resync(harness, 1_000_000_160_000)
    expect(records(harness)['team-a:A']).toEqual(expect.objectContaining({ state: 'sent' }))
    const keys = bodies(harness).map(body => body.idempotencyKey)
    expect(keys).toEqual(['c1:team-a:A', 'c1:team-a:A'])
  })

  it('stops the dispatch queue when the scope dies mid-round, keeping the rest pending', async () => {
    const harness = createHarness()
    await start(harness)
    harness.rankPages = [
      pageFixture(
        [
          rowFixture({ cells: { A: cellFixture() } }),
          rowFixture({ rank: 2, teamKey: 'team-b', cells: { B: cellFixture({ displayId: 'B' }) } }),
        ],
        false,
      ),
    ]
    harness.httpPlan = ['ok', 'context_expired']
    await resync(harness, 1_000_000_100_000)
    expect(records(harness)['team-a:A']).toEqual(expect.objectContaining({ state: 'sent' }))
    expect(records(harness)['team-b:B']).toEqual(expect.objectContaining({ state: 'pending' }))
    expect(harness.logs.some(line => line.includes('stay pending'))).toBe(true)
  })

  it('never backfills first-ACs hidden by the seal window after the reveal', async () => {
    const harness = createHarness()
    await start(harness)
    // Sealed round: remembers the window start.
    harness.contest = contestFixture({ sealed: true, sealedSince: 1_000_000_050_000 })
    await resync(harness, 1_000_000_060_000)
    // Reveal round: acTime 100s → inside [seal start, reveal); acTime 300s → after.
    harness.contest = contestFixture()
    harness.rankPages = [
      pageFixture([
        rowFixture({ teamKey: 'team-x', cells: { A: cellFixture({ acTime: 100 }) } }),
        rowFixture({ rank: 2, teamKey: 'team-y', cells: { B: cellFixture({ displayId: 'B', acTime: 300 }) } }),
      ]),
    ]
    await resync(harness, 1_000_000_200_000)
    expect(bodies(harness).map(body => body.idempotencyKey)).toEqual(['c1:team-y:B'])
    expect(harness.logs.some(line => line.includes('seal-skipped 1'))).toBe(true)
    // The skipped first-AC stays skipped on later rounds.
    harness.rankPages = [
      pageFixture([
        rowFixture({ teamKey: 'team-x', cells: { A: cellFixture({ acTime: 100 }) } }),
        rowFixture({ rank: 2, teamKey: 'team-y', cells: { B: cellFixture({ displayId: 'B', acTime: 300 }) } }),
      ]),
    ]
    await resync(harness, 1_000_000_260_000)
    expect(harness.requests.length).toBe(1)
    expect(records(harness)['team-x:A']).toBeUndefined()
  })

  it('stops the round on context_expired without throwing', async () => {
    const harness = createHarness()
    await start(harness)
    harness.contextMode = 'expired'
    await resync(harness, 1_000_000_100_000)
    expect(harness.logs.some(line => line.includes('context expired'))).toBe(true)
    expect(harness.requests).toEqual([])
    // Mid-round variant: the board fetch itself reports the expired scope.
    harness.contextMode = 'ok'
    harness.rankError = { code: 'context_expired' }
    await resync(harness, 1_000_000_160_000)
    expect(harness.requests).toEqual([])
    expect(harness.logs.filter(line => line.includes('context expired')).length).toBe(2)
  })

  it('aborts the round on budget exhaustion and retries on the next resync', async () => {
    const harness = createHarness()
    await start(harness)
    harness.rankPages = [pageFixture([rowFixture({ cells: { A: cellFixture() } })])]
    harness.rankError = { code: 'limit_exceeded' }
    await resync(harness, 1_000_000_100_000)
    expect(harness.logs.some(line => line.includes('limit_exceeded'))).toBe(true)
    expect(harness.requests).toEqual([])
    harness.rankError = null
    await resync(harness, 1_000_000_160_000)
    expect(records(harness)['team-a:A']).toEqual(expect.objectContaining({ state: 'sent' }))
  })

  it('isolates records per contest: the same team and problem dispatch again after a switch', async () => {
    const harness = createHarness()
    await start(harness)
    harness.rankPages = [pageFixture([rowFixture({ cells: { A: cellFixture() } })])]
    await resync(harness, 1_000_000_100_000)
    harness.contest = contestFixture({ contestId: 'c2' })
    harness.rankPages = [pageFixture([rowFixture({ cells: { A: cellFixture() } })])]
    await resync(harness, 1_000_000_160_000)
    const keys = bodies(harness).map(body => body.idempotencyKey)
    expect(keys).toEqual(['c1:team-a:A', 'c2:team-a:A'])
    expect(Object.keys(records(harness, 'c2'))).toEqual(['team-a:A'])
  })
})
