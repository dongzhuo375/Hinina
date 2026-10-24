import { beforeEach, describe, expect, it, vi } from 'vitest'
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

/**
 * Logic tests for the reference balloon plugin: the Hinina surface is mocked
 * (same layering rule as the frontend — tests mock the layer below), so
 * nothing touches a real host, OJ, or network.
 *
 * The mock enforces the host-side rules the plugin must live with, not just
 * happy paths: the request-header blacklist and URL acceptance of
 * infra/plugin_http (a forbidden header or a userinfo/fragment URL rejects
 * with invalid_input), the 6 rank queries/minute per-activation budget, and
 * the 30s handler deadline (host calls past it return timeout, which is how
 * the real runtime kills a round mid-dispatch).
 *
 * The plugin keeps its paging-budget state in module state with an
 * activation-long lifetime (see index.ts). The real host evaluates a fresh
 * module per activation, so each test re-imports a fresh module instance
 * instead of sharing one across the file.
 */

type Activate = (hinina: Hinina, config: Readonly<JsonValue>) => Promise<void>

let activate: Activate

beforeEach(async () => {
  vi.resetModules()
  ;({ default: activate } = await import('../index'))
})

type ResyncEvent = { type: 'resync.required'; lost: number }
type ResyncEnvelope = PluginEventEnvelope<ResyncEvent>
type ResyncHandler = (event: ResyncEvent, envelope: ResyncEnvelope) => void | Promise<void>
type HttpOutcome = 'ok' | 'http500' | 'timeout' | 'context_expired'
type ProbeOutcome = 'ok' | 'denied' | 'invalid' | 'unavailable'

interface Harness {
  hinina: Hinina
  /** `${level} ${message}` per written line. */
  logs: string[]
  /** Ordered capability calls (probe traffic excluded). */
  calls: string[]
  requests: PluginHttpRequest[]
  /** Activation-time GET probes against the endpoint (real context). */
  probeRequests: PluginHttpRequest[]
  storage: Map<string, JsonValue>
  handlers: ResyncHandler[]
  approved: Set<string>
  contest: PluginContest | null
  problems: PluginContestProblem[]
  rankPages: PluginRankPage[]
  rankError: { code: string } | null
  /** Rank query budget: burst size AND per-minute rate (governor Quota::per_minute). */
  rankQuota: number
  rankLimiter: Limiter
  /** Envelope clock; the fake host's rolling windows advance with it. */
  now: number
  /** Rank queries that reached the budget, in order (page/limit as requested). */
  rankQueries: PluginRankQuery[]
  contextMode: 'ok' | 'expired'
  httpPlan: HttpOutcome[]
  /** Host-policy rejection thrown from every dispatch, before any send. */
  httpError: { code: string; permission?: string } | null
  /** Activation-time endpoint probe outcome. */
  endpointProbe: ProbeOutcome
  /** Log outlet budget per harness (the host allows 30/min); excess calls reject. */
  logQuota: number
  logWritten: number
  /** Every log call, written or rejected — proves the round kept reporting. */
  logAttempts: string[]
  /** Storage op budget: burst size AND per-minute rate (governor Quota::per_minute). */
  storageQuota: number
  storageLimiter: Limiter
  /** Simulate the 30s handler deadline: once a dispatch times out, every later host call returns timeout. */
  killOnTimeout: boolean
  deadlinePassed: boolean
}

const ALL_PERMISSIONS = [
  'events.subscribe',
  'contest.read',
  'contest.rank',
  'storage.read',
  'storage.write',
  'network.request',
]

/** Header names the host rejects on plugin requests (api/http.rs blacklist). */
const FORBIDDEN_HEADERS = new Set([
  'host',
  'cookie',
  'connection',
  'upgrade',
  'transfer-encoding',
  'content-length',
  'content-type',
])

/**
 * Fractional token bucket mirroring the host's governor GCRA
 * (`Quota::per_minute(n)` = burst n cells, one cell replenished every
 * 60_000/n ms; see src-tauri/src/plugin/limits.rs and the governor crate).
 * The clock is the envelope time the tests control: the envelope's
 * occurredAt IS the host wall clock at adaptation time (event_adapter
 * wrap()), so the only real-world skew is the queueing delay until the
 * handler runs — individual tests model that skew by pre-draining the host
 * bucket (rankLimiter.tokens) below the plugin's local one.
 */
interface Limiter {
  tokens: number
  last: number
}

function admit(limiter: Limiter, quota: number, now: number): boolean {
  const period = 60_000 / quota
  limiter.tokens = Math.min(quota, limiter.tokens + (now - limiter.last) / period)
  limiter.last = now
  if (limiter.tokens < 1) return false
  limiter.tokens -= 1
  return true
}

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

/** A board of `teams` one-team pages, for the paging-cursor scenarios. */
function boardPages(teams: number): PluginRankPage[] {
  return Array.from({ length: teams }, (_, index) =>
    pageFixture(
      [
        rowFixture({
          rank: index + 1,
          teamKey: `team-${index + 1}`,
          displayName: `Team ${index + 1}`,
          cells: { A: cellFixture() },
        }),
      ],
      index < teams - 1,
    ),
  )
}

function createHarness(): Harness {
  const logs: string[] = []
  const calls: string[] = []
  const requests: PluginHttpRequest[] = []
  const probeRequests: PluginHttpRequest[] = []
  const storage = new Map<string, JsonValue>()
  const handlers: ResyncHandler[] = []
  const harness: Harness = {
    hinina: null as unknown as Hinina,
    logs,
    calls,
    requests,
    probeRequests,
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
    rankQuota: 6,
    rankLimiter: { tokens: 6, last: 0 },
    now: 0,
    rankQueries: [],
    contextMode: 'ok',
    httpPlan: [],
    httpError: null,
    endpointProbe: 'ok',
    logQuota: 30,
    logWritten: 0,
    logAttempts: [],
    storageQuota: 60,
    storageLimiter: { tokens: 60, last: 0 },
    killOnTimeout: false,
    deadlinePassed: false,
  }
  const frozen = (code: string): { code: string } => Object.freeze({ code })
  const permissionError = (permission: string) => Object.freeze({ code: 'permission_denied', permission })
  const stored = (key: string): PluginStorageValue =>
    storage.has(key) ? { state: 'value', value: storage.get(key) as JsonValue } : { state: 'missing' }
  const deadline = (): void => {
    if (harness.deadlinePassed) throw frozen('timeout')
  }
  /** Mirrors the host's request checks: header blacklist + userinfo/fragment URL rejection. */
  const validateRequest = (request: PluginHttpRequest): void => {
    for (const [name, value] of Object.entries(request.headers ?? {})) {
      const lower = name.toLowerCase()
      if (
        name === '' ||
        !/^[A-Za-z0-9-]+$/.test(name) ||
        FORBIDDEN_HEADERS.has(lower) ||
        lower.startsWith('proxy-') ||
        /[\x00-\x1f]/.test(value)
      ) {
        throw frozen('invalid_input')
      }
    }
    let url: URL
    try {
      url = new URL(request.url)
    } catch {
      throw frozen('invalid_input')
    }
    if (url.username !== '' || url.password !== '' || url.hash !== '') throw frozen('invalid_input')
  }
  /** Log outlet: rejects beyond the quota (limit_exceeded), like the host's 30/min. */
  const writeLog = (level: string, message: string): void => {
    deadline()
    harness.logAttempts.push(`${level} ${message}`)
    if (harness.logWritten >= harness.logQuota) throw frozen('limit_exceeded')
    harness.logWritten += 1
    logs.push(`${level} ${message}`)
  }
  /** Storage transaction gate: every op (get/set/CAS/delete) shares the GCRA budget. */
  const storageOp = (): void => {
    if (!admit(harness.storageLimiter, harness.storageQuota, harness.now)) throw frozen('limit_exceeded')
  }

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
        if (harness.rankError !== null) throw frozen(harness.rankError.code)
        // Out-of-range page/limit rejects with limit_exceeded before any
        // budget charge, like PluginRankQuery::validate.
        if (query.page < 1 || query.page > 100 || query.limit < 1 || query.limit > 200) {
          throw frozen('limit_exceeded')
        }
        // GCRA budget, charged per board fetch: burst = rankQuota cells, one
        // cell replenished every 60s/rankQuota (governor Quota::per_minute).
        // Content-rejected pages keep their charge, like the host.
        if (!admit(harness.rankLimiter, harness.rankQuota, harness.now)) throw frozen('limit_exceeded')
        calls.push(`rank:${query.page}`)
        harness.rankQueries.push(query)
        // Pages are indexed by position: the paging cursor asks for arbitrary
        // page numbers (and wraps back to 1), not a FIFO sequence.
        const page = harness.rankPages[query.page - 1]
        if (page === undefined) return { rows: [], page: query.page, hasMore: false }
        // Content bounds, checked after the charge (mirrors the host): a
        // page over 200 rows or 6,000 cells is refused — e.g. Hydro returns
        // its whole board as one page regardless of the requested limit.
        if (page.rows.length > 200) throw frozen('limit_exceeded')
        const cells = page.rows.reduce((total, row) => total + Object.keys(row.cells).length, 0)
        if (cells > 6000) throw frozen('limit_exceeded')
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
      get: async (key: string) => {
        storageOp()
        return stored(key)
      },
      set: async (key: string, value: JsonValue) => {
        deadline()
        if (key === 'balloon.probe') return
        storageOp()
        calls.push(`set:${key}`)
        storage.set(key, value)
      },
      delete: async (key: string) => {
        storageOp()
        storage.delete(key)
      },
      compareAndSet: async (key: string, expected: PluginStorageValue, value: JsonValue) => {
        if (!harness.approved.has('storage.read')) throw permissionError('storage.read')
        if (!harness.approved.has('storage.write')) throw permissionError('storage.write')
        storageOp()
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
        deadline()
        validateRequest(request)
        if (request.method === 'GET') {
          // Activation-time endpoint probe: side-effect free, not dispatch traffic.
          probeRequests.push(request)
          if (harness.endpointProbe === 'denied') throw permissionError('network.request')
          if (harness.endpointProbe === 'invalid') throw frozen('invalid_input')
          if (harness.endpointProbe === 'unavailable') throw frozen('unavailable')
          return { ok: true, status: 200, headers: {}, body: '' }
        }
        if (harness.httpError !== null) throw Object.freeze({ ...harness.httpError })
        requests.push(request)
        calls.push('http')
        const outcome = harness.httpPlan.shift() ?? 'ok'
        if (outcome === 'http500') return { ok: false, status: 500, headers: {}, body: '' }
        if (outcome === 'timeout') {
          if (harness.killOnTimeout) harness.deadlinePassed = true
          throw frozen('timeout')
        }
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
      debug: async (message: string) => writeLog('debug', message),
      info: async (message: string) => writeLog('info', message),
      warn: async (message: string) => writeLog('warn', message),
      error: async (message: string) => writeLog('error', message),
    },
  }
  harness.hinina = hinina as unknown as Hinina
  return harness
}

async function start(harness: Harness, endpoint = 'https://httpbin.org/post'): Promise<void> {
  await activate(harness.hinina, { endpoint })
}

async function resync(harness: Harness, now: number): Promise<void> {
  harness.now = now
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

describe('balloon reference plugin host contract', () => {
  it('keeps the copied header blacklist in sync with api/http.rs', async () => {
    // Variable specifier: the repo has no @types/node for tsc to resolve;
    // the import only needs to work under vitest, which runs in Node.
    const fsModule = 'node:fs'
    const { readFileSync } = (await import(fsModule)) as {
      readFileSync: (path: URL, encoding: string) => string
    }
    const source = readFileSync(
      new URL('../../../../../../../../src-tauri/src/plugin/api/http.rs', import.meta.url),
      'utf-8',
    )
    // FORBIDDEN_HEADERS above is a hand copy of the host's blacklist — if
    // the host list changes without the copy (or vice versa), this fails.
    const blacklist = source.match(/matches!\(\s*lower\.as_str\(\)\s*,\s*((?:"[a-z-]+"\s*\|?\s*)+)\)/)
    expect(blacklist).not.toBeNull()
    const names = [...blacklist![1].matchAll(/"([a-z-]+)"/g)].map(match => match[1])
    expect([...new Set(names)].sort()).toEqual([...FORBIDDEN_HEADERS].sort())
    // The proxy- prefix rule belongs to the same blacklist behavior.
    expect(source).toMatch(/lower\.starts_with\("proxy-"\)/)
  })
})

describe('balloon reference plugin activation', () => {
  it('registers the resync handler and probes the endpoint with one GET', async () => {
    const harness = createHarness()
    await start(harness)
    expect(harness.handlers.length).toBe(1)
    // The probes are side-effect free: no rank page consumed, nothing written,
    // and the endpoint probe is a GET, never a dispatch.
    expect(harness.calls).toEqual([])
    expect(harness.requests).toEqual([])
    expect(harness.probeRequests.map(request => [request.method, request.url])).toEqual([
      ['GET', 'https://httpbin.org/post'],
    ])
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
    // The host state only records script_error; the log must name the cause.
    expect(
      harness.logs.some(line => line.includes('activation failed at contest.rank probe: permission_denied{contest.rank}')),
    ).toBe(true)
  })

  it('rejects endpoint options the host would refuse, and accepts a query string', async () => {
    const harness = createHarness()
    await expect(activate(harness.hinina, {})).rejects.toMatchObject({ code: 'invalid_input' })
    await expect(activate(harness.hinina, { endpoint: 'http://insecure.example' })).rejects.toMatchObject({
      code: 'invalid_input',
    })
    // Userinfo and fragment are rejected by the host's send path.
    await expect(activate(harness.hinina, { endpoint: 'https://user:pass@example.com/p' })).rejects.toMatchObject({
      code: 'invalid_input',
    })
    await expect(activate(harness.hinina, { endpoint: 'https://example.com/p#f' })).rejects.toMatchObject({
      code: 'invalid_input',
    })
    // A query string is legal for the host and must not fail activation.
    await activate(harness.hinina, { endpoint: 'https://example.com/p?x=1' })
    expect(harness.handlers.length).toBe(1)
  })

  it('fails activation when the endpoint origin is not approved and a context exists', async () => {
    const harness = createHarness()
    harness.endpointProbe = 'denied'
    await expect(start(harness)).rejects.toMatchObject({
      code: 'permission_denied',
      permission: 'network.request',
    })
    expect(harness.handlers.length).toBe(0)
    expect(harness.logs.some(line => line.includes('activation failed at endpoint probe (permission_denied)'))).toBe(true)
  })

  it('fails activation when the host rejects the endpoint URL', async () => {
    const harness = createHarness()
    harness.endpointProbe = 'invalid'
    await expect(start(harness)).rejects.toMatchObject({ code: 'invalid_input' })
    expect(harness.logs.some(line => line.includes('activation failed at endpoint probe (invalid_input)'))).toBe(true)
  })

  it('skips the endpoint probe when no query context exists, and checks it on dispatch instead', async () => {
    const harness = createHarness()
    harness.contextMode = 'expired'
    await start(harness)
    expect(harness.handlers.length).toBe(1)
    expect(harness.probeRequests).toEqual([])
    expect(harness.logs.some(line => line.includes('no query context at activation'))).toBe(true)
    // Later rounds have a context and dispatch normally.
    harness.contextMode = 'ok'
    harness.rankPages = [pageFixture([rowFixture({ cells: { A: cellFixture() } })])]
    await resync(harness, 1_000_000_100_000)
    expect(records(harness)['team-a:A']).toEqual(expect.objectContaining({ state: 'sent' }))
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

  it('claims pending before dispatching, sends no blacklisted headers, and marks sent only after a 2xx', async () => {
    const harness = createHarness()
    await start(harness)
    harness.rankPages = [pageFixture([rowFixture({ cells: { A: cellFixture() } })])]
    await resync(harness, 1_000_000_100_000)
    expect(harness.calls.indexOf('cas:balloon.records:c1')).toBeLessThan(harness.calls.indexOf('http'))
    // The host blacklists content-type (it sets application/json itself);
    // a request carrying it would be rejected with invalid_input by the mock.
    for (const request of harness.requests) {
      expect(Object.keys(request.headers ?? {}).map(name => name.toLowerCase())).not.toContain('content-type')
    }
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

  it('persists every sent balloon immediately, so a deadline kill keeps confirmed dispatches', async () => {
    const harness = createHarness()
    await start(harness)
    harness.rankPages = [
      pageFixture([
        rowFixture({ cells: { A: cellFixture() } }),
        rowFixture({ rank: 2, teamKey: 'team-b', cells: { B: cellFixture({ displayId: 'B' }) } }),
        rowFixture({ rank: 3, teamKey: 'team-c', cells: { C: cellFixture({ displayId: 'C' }) } }),
      ]),
    ]
    // The third dispatch crosses the 30s handler deadline: from there on the
    // host answers every call with timeout, exactly like the real runtime.
    harness.killOnTimeout = true
    harness.httpPlan = ['ok', 'ok', 'timeout']
    await resync(harness, 1_000_000_100_000)
    // The two confirmed dispatches are already on disk; only the third is
    // pending. The timed-out request was still sent (at-least-once), so the
    // receiver may see it again next round under the same idempotency key.
    expect(records(harness)['team-a:A']).toEqual(expect.objectContaining({ state: 'sent' }))
    expect(records(harness)['team-b:B']).toEqual(expect.objectContaining({ state: 'sent' }))
    expect(records(harness)['team-c:C']).toEqual(expect.objectContaining({ state: 'pending' }))
    expect(harness.requests.length).toBe(3)
    expect(harness.calls.filter(call => call.startsWith('set:balloon.records')).length).toBe(2)
  })

  it('never aborts a round when the log outlet is exhausted (note() swallows)', async () => {
    const harness = createHarness()
    // Every outlet call rejects with limit_exceeded, like a round that starts
    // with the 30/min log budget already spent. note() must swallow each one:
    // the round still dispatches, persists, and keeps reporting — it must not
    // be misreported as aborted.
    harness.logQuota = 0
    await start(harness)
    harness.rankPages = [pageFixture([rowFixture({ cells: { A: cellFixture() } })])]
    await resync(harness, 1_000_000_100_000)
    expect(records(harness)['team-a:A']).toEqual(expect.objectContaining({ state: 'sent' }))
    expect(harness.logs).toEqual([])
    expect(harness.logAttempts.some(line => line.includes('dispatched 1'))).toBe(true)
    expect(harness.logAttempts.some(line => line.includes('aborted'))).toBe(false)
  })

  it('converges over rounds when the storage op quota runs out mid-queue', async () => {
    const harness = createHarness()
    // Same envelope time for activation and the first round, so the probe's
    // CAS shares the storage window with the round (the real host sees both
    // within one wall-clock minute).
    harness.now = 1_000_000_100_000
    await start(harness)
    // 60 balloons on one page. Storage ops (get/set/CAS/delete) share the
    // host's 60/min quota; the claim is ONE batch CAS, each success one set.
    // After the probe CAS, two gets, and the claim, exactly 56 sets fit the
    // window; the 57th balloon is POSTed but its set is rejected.
    const rows = Array.from({ length: 60 }, (_, index) =>
      rowFixture({ rank: index + 1, teamKey: `team-${index + 1}`, cells: { A: cellFixture() } }),
    )
    harness.rankPages = [pageFixture(rows)]
    await resync(harness, 1_000_000_100_000)
    const afterFirst = Object.values(records(harness))
    expect(afterFirst.filter(record => record.state === 'sent').length).toBe(56)
    expect(afterFirst.filter(record => record.state === 'pending').length).toBe(4)
    expect(harness.logs.some(line => line.includes('persisting sent failed (limit_exceeded)'))).toBe(true)
    expect(harness.requests.length).toBe(57)
    // A fresh minute finishes the pending retries under the same idempotency
    // keys — at-least-once: the POSTed-but-unpersisted balloon is re-sent.
    await resync(harness, 1_000_000_160_000)
    expect(Object.values(records(harness)).every(record => record.state === 'sent')).toBe(true)
    expect(harness.requests.length).toBe(61)
    const keys = bodies(harness).map(body => body.idempotencyKey)
    expect(new Set(keys).size).toBe(60)
  })

  it('treats an endpoint policy rejection as dead instead of retrying forever', async () => {
    const harness = createHarness()
    await start(harness)
    harness.rankPages = [
      pageFixture([
        rowFixture({ cells: { A: cellFixture() } }),
        rowFixture({ rank: 2, teamKey: 'team-b', cells: { B: cellFixture({ displayId: 'B' }) } }),
      ]),
    ]
    // Origin approved at activation, revoked at dispatch (config drift).
    harness.httpError = { code: 'permission_denied', permission: 'network.request' }
    await resync(harness, 1_000_000_100_000)
    expect(harness.requests).toEqual([])
    expect(records(harness)['team-a:A']).toEqual(expect.objectContaining({ state: 'pending' }))
    expect(records(harness)['team-b:B']).toEqual(expect.objectContaining({ state: 'pending' }))
    expect(harness.logs.some(line => line.includes('queue stopped') && line.includes('permission_denied{network.request}'))).toBe(true)
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

  it('discloses the missing seal memory on a cold start and dispatches as fresh', async () => {
    const harness = createHarness()
    await start(harness)
    // No sealed round was ever observed: the plugin cannot apply the
    // no-backfill rule and must say so instead of dispatching silently.
    harness.rankPages = [pageFixture([rowFixture({ cells: { A: cellFixture({ acTime: 100 }) } })])]
    await resync(harness, 1_000_000_100_000)
    expect(harness.logs.some(line => line.includes('no seal memory'))).toBe(true)
    expect(records(harness)['team-a:A']).toEqual(expect.objectContaining({ state: 'sent' }))
  })

  it('retries a budget-rejected page once before anything else, then recovers', async () => {
    const harness = createHarness()
    await start(harness)
    harness.rankQuota = 2
    const pages = (): PluginRankPage[] => [
      pageFixture([rowFixture({ cells: { A: cellFixture() } })], true),
      pageFixture([rowFixture({ rank: 2, teamKey: 'team-b', cells: { B: cellFixture({ displayId: 'B' }) } })], true),
      pageFixture([rowFixture({ rank: 3, teamKey: 'team-c', cells: { C: cellFixture({ displayId: 'C' }) } })], false),
    ]
    harness.rankPages = pages()
    await resync(harness, 1_000_000_100_000)
    // Pages 1-2 are fetched and dispatched; page 3 is quota-rejected on its
    // FIRST attempt — the cursor stays on it, nothing is skipped, and the
    // round does not abort.
    expect(harness.calls).toEqual([
      'problems',
      'rank:1',
      'rank:2',
      'cas:balloon.records:c1',
      'http',
      'set:balloon.records:c1',
      'http',
      'set:balloon.records:c1',
    ])
    expect(records(harness)['team-a:A']).toEqual(expect.objectContaining({ state: 'sent' }))
    expect(records(harness)['team-b:B']).toEqual(expect.objectContaining({ state: 'sent' }))
    expect(records(harness)['team-c:C']).toBeUndefined()
    expect(
      harness.logs.some(line => line.includes('rank page 3 rejected (limit_exceeded), window 1 of 3; retrying the same page')),
    ).toBe(true)
    expect(harness.logs.some(line => line.includes('consecutive windows'))).toBe(false)
    // Budget recovered (e.g. the shared provider budget freed up): the retry
    // of the SAME page succeeds — no page was sacrificed.
    harness.rankQuota = 6
    await resync(harness, 1_000_000_130_000)
    expect(harness.calls.filter(call => call.startsWith('rank:'))).toEqual(['rank:1', 'rank:2', 'rank:3'])
    expect(bodies(harness).map(body => body.idempotencyKey)).toEqual([
      'c1:team-a:A',
      'c1:team-b:B',
      'c1:team-c:C',
    ])
    expect(records(harness)['team-c:C']).toEqual(expect.objectContaining({ state: 'sent' }))
  })

  it('skips a page rejected across 3 consecutive windows instead of pinning the cursor', async () => {
    const harness = createHarness()
    await start(harness)
    // Page 2 is oversized: 31 rows × 194 columns = 6,014 cells, over the
    // host's 6,000-cell page bound. The page that comes back is bigger than
    // any requested limit can fix (Hydro behaves this way — it returns the
    // whole board as one page), so every fetch of it is limit_exceeded.
    const padding: Record<string, PluginRankCell> = {}
    for (let column = 0; column < 193; column += 1) {
      padding[`X${column}`] = cellFixture({ displayId: `X${column}`, firstAc: false })
    }
    harness.rankPages = [
      pageFixture([rowFixture({ cells: { A: cellFixture() } })], true),
      pageFixture(
        Array.from({ length: 31 }, (_, index) =>
          rowFixture({ rank: index + 1, teamKey: `team-p2-${index}`, cells: { A: cellFixture(), ...padding } }),
        ),
        true,
      ),
      pageFixture([rowFixture({ rank: 2, teamKey: 'team-c', cells: { C: cellFixture({ displayId: 'C' }) } })], false),
    ]
    // Window 1: page 1 dispatches; page 2 is rejected (count 1).
    await resync(harness, 1_000_000_100_000)
    expect(harness.calls.filter(call => call.startsWith('rank:'))).toEqual(['rank:1', 'rank:2'])
    expect(harness.logs.some(line => line.includes('rank page 2 rejected (limit_exceeded), window 1 of 3'))).toBe(true)
    expect(records(harness)['team-a:A']).toEqual(expect.objectContaining({ state: 'sent' }))
    // Same window, next round: the retry is rejected again but the window
    // count does NOT advance — a same-window retry is not a second window.
    await resync(harness, 1_000_000_130_000)
    expect(harness.calls.filter(call => call.startsWith('rank:')).slice(-1)).toEqual(['rank:2'])
    expect(harness.logs.filter(line => line.includes('window 1 of 3')).length).toBe(2)
    // Window 2: rejected again, count advances to 2 — still retrying.
    await resync(harness, 1_000_000_160_000)
    expect(harness.logs.some(line => line.includes('rank page 2 rejected (limit_exceeded), window 2 of 3'))).toBe(true)
    expect(harness.logs.some(line => line.includes('consecutive windows'))).toBe(false)
    // Window 3: the threshold is reached — the page is skipped and page 3 is
    // reached in the SAME round. The cursor is not pinned forever.
    await resync(harness, 1_000_000_220_000)
    expect(harness.calls.filter(call => call.startsWith('rank:'))).toEqual([
      'rank:1',
      'rank:2',
      'rank:2',
      'rank:2',
      'rank:2',
      'rank:3',
    ])
    expect(
      harness.logs.some(line => line.includes('rank page 2 rejected in 3 consecutive windows') && line.includes('skipping it')),
    ).toBe(true)
    expect(records(harness)['team-c:C']).toEqual(expect.objectContaining({ state: 'sent' }))
    // The oversized page's teams get no balloons: the host refuses the page.
    expect(records(harness)['team-p2-0:A']).toBeUndefined()
  })

  it('adapts the page limit to the problem count so pages stay encodable', async () => {
    const harness = createHarness()
    // 40 problems: a fixed 200-row page would be 200 × 40 = 8,000 cells and
    // rejected on content grounds. The adaptive limit is
    // floor(6,000 / (40 + 1 spare column)) = 146 rows per page.
    harness.problems = Array.from({ length: 40 }, (_, index) => ({
      problemId: `P${index + 1}`,
      displayId: `Q${index}`,
      title: `Problem ${index}`,
      color: 'red',
    }))
    await start(harness)
    harness.rankPages = [pageFixture([rowFixture({ cells: { Q0: cellFixture({ displayId: 'Q0' }) } })])]
    await resync(harness, 1_000_000_100_000)
    expect(harness.rankQueries.map(query => query.limit)).toEqual([146])
    expect(records(harness)['team-a:Q0']).toEqual(expect.objectContaining({ state: 'sent' }))
    // Duplicate displayIds count by LIST length, not the Map size: a
    // duplicated id must shrink the page (40 entries -> floor(6000/41) =
    // 146), not slip past the cell bound via the Map's 39 unique keys
    // (which would give 150 rows x 40 columns = 6,000 cells with zero
    // headroom for the duplicate's board column).
    const duplicated = createHarness()
    duplicated.problems = [
      ...Array.from({ length: 39 }, (_, index) => ({
        problemId: `P${index + 1}`,
        displayId: `Q${index}`,
        title: `Problem ${index}`,
        color: 'red',
      })),
      { problemId: 'P40-dup', displayId: 'Q0', title: 'Problem 0 duplicate', color: 'red' },
    ]
    await start(duplicated)
    duplicated.rankPages = [pageFixture([rowFixture({ cells: { Q0: cellFixture({ displayId: 'Q0' }) } })])]
    await resync(duplicated, 1_000_000_100_000)
    expect(duplicated.rankQueries.map(query => query.limit)).toEqual([146])
    // Small contests keep the full 200-row page.
    const small = createHarness()
    await start(small)
    small.rankPages = [pageFixture([rowFixture({ cells: { A: cellFixture() } })])]
    await resync(small, 1_000_000_100_000)
    expect(small.rankQueries.map(query => query.limit)).toEqual([200])
  })

  it('refills the rank budget continuously and rotates the board with a cursor', async () => {
    const harness = createHarness()
    await start(harness)
    // An 8-page board; host and plugin both run GCRA (burst 6, 1 token/10s).
    harness.rankPages = boardPages(8)
    await resync(harness, 1_000_000_100_000)
    // Round 1 spends the full burst: pages 1-6, no per-round cap.
    expect(harness.calls.filter(call => call.startsWith('rank:'))).toEqual([
      'rank:1',
      'rank:2',
      'rank:3',
      'rank:4',
      'rank:5',
      'rank:6',
    ])
    expect(harness.requests.length).toBe(6)
    // Round 2, 30s later: 3 tokens refilled — the cursor continues at page
    // 7, wraps at the board end, and all 8 teams are dispatched.
    await resync(harness, 1_000_000_130_000)
    expect(harness.calls.filter(call => call.startsWith('rank:'))).toEqual([
      'rank:1',
      'rank:2',
      'rank:3',
      'rank:4',
      'rank:5',
      'rank:6',
      'rank:7',
      'rank:8',
    ])
    expect(bodies(harness).map(body => body.teamKey)).toEqual([
      'team-1',
      'team-2',
      'team-3',
      'team-4',
      'team-5',
      'team-6',
      'team-7',
      'team-8',
    ])
    // Round 3, 30s later: 4 tokens — the rotation restarts at page 1 and
    // re-scans; nothing new to dispatch, nothing lost.
    await resync(harness, 1_000_000_160_000)
    expect(harness.calls.filter(call => call.startsWith('rank:'))).toEqual([
      'rank:1',
      'rank:2',
      'rank:3',
      'rank:4',
      'rank:5',
      'rank:6',
      'rank:7',
      'rank:8',
      'rank:1',
      'rank:2',
      'rank:3',
      'rank:4',
    ])
    expect(harness.requests.length).toBe(8)
    expect(harness.logs.some(line => line.includes('nothing to do'))).toBe(true)
  })

  it('never over-claims the host budget at a window boundary (the counterexample schedule)', async () => {
    const harness = createHarness()
    await start(harness)
    // The minimal counterexample to a fixed-60s-window ledger, replayed
    // exactly (times relative to the first round):
    //   t=0s   1 call    window 1 starts (ledger spends 1 of 6)
    //   t=51s  5 calls   still window 1 (ledger now full: 6 spent)
    //   t=60s  round 3   a fixed window RESETS to a fresh 6 and attempts 6
    //                    more; the host GCRA, 9s after the last spend, has
    //                    0.9 tokens and rejects 5 of them. The local GCRA
    //                    bucket has the same 1.9 tokens, spends 1, and
    //                    nothing is rejected.
    harness.rankPages = boardPages(1)
    await resync(harness, 1_000_000_100_000)
    harness.rankPages = boardPages(5)
    await resync(harness, 1_000_000_151_000)
    await resync(harness, 1_000_000_160_000)
    expect(harness.calls.filter(call => call.startsWith('rank:'))).toEqual([
      'rank:1',
      'rank:1',
      'rank:2',
      'rank:3',
      'rank:4',
      'rank:5',
      'rank:1',
    ])
    expect(harness.logs.some(line => line.includes('rejected'))).toBe(false)
    // All five teams of the board are dispatched by round 2.
    for (let team = 1; team <= 5; team += 1) {
      expect(records(harness)[`team-${team}:A`]).toEqual(expect.objectContaining({ state: 'sent' }))
    }
    // A minute later the burst has refilled and the rotation continues
    // cleanly (pages 2-5 re-scanned, nothing new, nothing rejected).
    harness.rankPages = boardPages(5)
    await resync(harness, 1_000_000_220_000)
    expect(harness.calls.filter(call => call.startsWith('rank:')).length).toBe(11)
    expect(harness.logs.some(line => line.includes('rejected'))).toBe(false)
  })

  it('absorbs a stricter host bucket: rejections retry, never skip, and heal', async () => {
    const harness = createHarness()
    // Model the queueing-delay skew: the host bucket sits BELOW the local
    // one (as if the envelope had waited in queue while the host spent
    // tokens). Rejections occur, the same page is retried, the skip
    // threshold is never reached, and the board completes within 3 rounds.
    // (last is pinned to the round time so the pre-drained tokens are not
    // refilled away on the first admit.)
    harness.now = 1_000_000_100_000
    await start(harness)
    harness.rankLimiter.tokens = 2
    harness.rankLimiter.last = 1_000_000_100_000
    harness.rankPages = boardPages(6)
    await resync(harness, 1_000_000_100_000)
    await resync(harness, 1_000_000_130_000)
    await resync(harness, 1_000_000_160_000)
    expect(harness.logs.filter(line => line.includes('window 1 of 3')).length).toBe(2)
    expect(harness.logs.some(line => line.includes('consecutive windows'))).toBe(false)
    for (let team = 1; team <= 6; team += 1) {
      expect(records(harness)[`team-${team}:A`]).toEqual(expect.objectContaining({ state: 'sent' }))
    }
  })

  it('skips board columns missing from the problem list until it catches up', async () => {
    const harness = createHarness()
    await start(harness)
    const board = (): PluginRankPage[] => [
      pageFixture([
        rowFixture({ cells: { A: cellFixture(), Z: cellFixture({ displayId: 'Z' }) } }),
      ]),
    ]
    harness.rankPages = board()
    await resync(harness, 1_000_000_100_000)
    // Z has no problem id or color: no useless empty balloon, no claim.
    expect(bodies(harness).map(body => body.idempotencyKey)).toEqual(['c1:team-a:A'])
    expect(records(harness)['team-a:Z']).toBeUndefined()
    expect(harness.logs.some(line => line.includes('missing from the problem list: Z'))).toBe(true)
    // Once the problem list includes Z, the cell is picked up again.
    harness.problems = [
      ...harness.problems,
      { problemId: 'PZ', displayId: 'Z', title: 'Problem Z', color: 'green' },
    ]
    harness.rankPages = board()
    await resync(harness, 1_000_000_160_000)
    expect(bodies(harness).map(body => body.idempotencyKey)).toEqual(['c1:team-a:A', 'c1:team-a:Z'])
    expect(records(harness)['team-a:Z']).toEqual(expect.objectContaining({ state: 'sent', problemId: 'PZ' }))
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
