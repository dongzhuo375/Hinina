import type {
  Hinina,
  JsonValue,
  PluginContest,
  PluginContestProblem,
  PluginLogLevel,
  PluginRankCell,
  PluginRankRow,
  PluginStorageValue,
} from '../../hinina'

/**
 * Reference first-AC balloon dispatcher (plugin construction plan, phase C).
 *
 * It exists to prove that the host capabilities compose into the target
 * workflow — log outlet + scoped queries + contest problems/rank + private
 * storage (CAS) + restricted HTTP — while every business rule stays in the
 * plugin: the host never learns what a "balloon" is.
 *
 * Round shape (every `resync.required`; the runtime has no setTimeout, so
 * Resync is the only heartbeat):
 *
 *   context.current → contest.current → sealed? stop the whole round →
 *   contest.problems → contest.rank page by page (dedupe rows by teamKey) →
 *   for every first-AC cell not yet sent: claim `pending` via CAS → POST to
 *   the configured endpoint with an idempotency key → mark `sent`.
 *
 * Storage layout (per contest, so switching contests cannot collide):
 *   `balloon.seal:<contestId>`    → { since, revealedAt } seal-window memory
 *   `balloon.records:<contestId>` → { [teamKey:displayId]: balloon record }
 */

/** Dispatch target; deployment-specific, passed via `plugins.json` options. */
interface BalloonOptions {
  endpoint: string
}

// Type aliases (not interfaces) so the JSON-shaped records satisfy the
// index-signature object variant of JsonValue when passed to storage.
type BalloonRecord = {
  state: 'pending' | 'sent'
  displayId: string
  problemId: string
  color: string
  teamKey: string
  displayName: string
  school: string
  rank: number
  /** Seconds since contest start, as reported by the board; null while sealed. */
  acTime: number | null
  /** Round time (envelope.occurredAt) that last wrote this record. */
  updatedAt: number
}

type SealWindow = {
  /** UTC ms when the seal started (contest.sealedSince). */
  since: number
  /**
   * UTC ms of the first round that saw the board unsealed again; null while
   * still sealed. First-ACs with an AC time inside [since, revealedAt) were
   * hidden by the seal and are never dispatched after the reveal.
   */
  revealedAt: number | null
}

type RecordMap = Record<string, BalloonRecord>

/** Contract bounds: page 1..100, at most 200 rows per page. */
const RANK_LIMIT = 200
const MAX_RANK_PAGES = 100
const RECORDS_KEY_PREFIX = 'balloon.records:'
const SEAL_KEY_PREFIX = 'balloon.seal:'
/** Cannot collide with a host-minted query-context handle. */
const PROBE_CONTEXT = '__balloon_capability_probe__'

/**
 * @plugin-entry
 * @plugin-id com.example.balloon
 * @runtime js
 * @api-version 2
 */
export default async function activate(hinina: Hinina, config: Readonly<JsonValue>): Promise<void> {
  const settings = readOptions(config)
  await probeCapabilities(hinina, settings)

  /**
   * @plugin-handler
   * @event resync.required
   * @idempotent true
   * @resync-safe true
   */
  hinina.events.on('resync.required', async (_event, envelope) => {
    await runRound(hinina, settings, envelope.occurredAt)
  })
}

/** Read and validate `options.endpoint` (https URL; plugin HTTP rejects loopback/private). */
function readOptions(config: Readonly<JsonValue>): BalloonOptions {
  if (config === null || typeof config !== 'object' || Array.isArray(config)) {
    throw Object.freeze({ code: 'invalid_input' })
  }
  const endpoint = (config as Record<string, JsonValue>).endpoint
  if (typeof endpoint !== 'string' || !/^https:\/\/[^\s/]+(\/[^\s]*)?$/.test(endpoint)) {
    throw Object.freeze({ code: 'invalid_input' })
  }
  return { endpoint }
}

/**
 * Fail fast at activation when an approved permission is missing: a plugin
 * that can never dispatch should land in `failed` with the missing permission
 * instead of silently no-op-ing every round. Every probe is side-effect free:
 * the dummy context fails the scope check before any provider call, and the
 * storage probe compares against a value that cannot exist (no write happens).
 */
async function probeCapabilities(hinina: Hinina, settings: BalloonOptions): Promise<void> {
  // contest.rank checks contest.rank THEN contest.read, then the scope check.
  await expectContextExpired(hinina.contest.rank(PROBE_CONTEXT, { page: 1, limit: 1 }))
  // compareAndSet checks storage.read + storage.write; a fresh key never
  // matches the expected value, so the comparison fails without writing.
  if (await hinina.storage.compareAndSet('balloon.probe', { state: 'value', value: null }, null)) {
    await hinina.storage.delete('balloon.probe')
  }
  // http.request checks network.request, then the shape, then the scope check.
  await expectContextExpired(
    hinina.http.request(PROBE_CONTEXT, { method: 'GET', url: settings.endpoint, timeoutMs: null }),
  )
}

/** A dummy context must fail the scope check; permission errors propagate. */
async function expectContextExpired(call: Promise<unknown>): Promise<void> {
  try {
    await call
  } catch (error) {
    if (errorCode(error) === 'context_expired') return
    throw error
  }
  throw Object.freeze({ code: 'script_error' })
}

async function runRound(hinina: Hinina, settings: BalloonOptions, now: number): Promise<void> {
  try {
    await round(hinina, settings, now)
  } catch (error) {
    const code = errorCode(error)
    if (code === 'context_expired') {
      // Login/logout, OJ or contest switch, connection change: stop this
      // round quietly; the next Resync re-reads a fresh context.
      await note(hinina, 'info', 'balloon: context expired; round stopped')
      return
    }
    if (
      code === 'limit_exceeded' ||
      code === 'timeout' ||
      code === 'provider_error' ||
      code === 'unauthenticated'
    ) {
      await note(hinina, 'warn', `balloon: round aborted (${code}); retrying next resync`)
      return
    }
    if (code === 'permission_denied') {
      // Impossible after the activation probe unless approvals changed
      // without a reload; report loudly instead of spinning silently.
      await note(hinina, 'error', 'balloon: permission denied; check plugins.json and reload')
      return
    }
    throw error
  }
}

async function round(hinina: Hinina, settings: BalloonOptions, now: number): Promise<void> {
  const context = await hinina.context.current()
  const contest = await hinina.contest.current(context)
  if (contest === null) {
    await hinina.log.debug('balloon: no contest selected; round skipped')
    return
  }

  const seal = await updateSealMemory(hinina, contest, now)
  if (contest.sealed) {
    // A sealed board withholds pass states: "not solved" is indistinguishable
    // from "solved during the seal". Never read the board, never dispatch —
    // otherwise the reveal would replay a batch of stale balloons.
    await hinina.log.info(`balloon: sealed since ${seal?.since ?? now}; round skipped`)
    return
  }

  const problems = await hinina.contest.problems(context)
  const problemOf = new Map(
    problems.map((problem): [string, PluginContestProblem] => [problem.displayId, problem]),
  )

  const recordsKey = RECORDS_KEY_PREFIX + contest.contestId
  const stored = await hinina.storage.get(recordsKey)
  const records = readRecordMap(stored)

  // Live view, no snapshot across pages: dedupe rows by teamKey within the
  // round; cross-round dedupe is the records map (key = teamKey:displayId).
  const seenTeams = new Set<string>()
  const fresh: Array<{ key: string; row: PluginRankRow; cell: PluginRankCell }> = []
  let sealSkipped = 0
  let pages = 0
  let truncated = false
  for (let page = 1; page <= MAX_RANK_PAGES; page += 1) {
    const board = await hinina.contest.rank(context, { page, limit: RANK_LIMIT })
    pages += 1
    for (const row of board.rows) {
      if (row.rank === -1) continue // starred (unranked) teams get no balloons
      if (seenTeams.has(row.teamKey)) continue
      seenTeams.add(row.teamKey)
      for (const cell of Object.values(row.cells)) {
        if (!cell.firstAc) continue
        const key = `${row.teamKey}:${cell.displayId}`
        if (records[key] !== undefined) continue // already claimed or sent
        if (isSealWindowAc(seal, contest, cell.acTime)) {
          sealSkipped += 1
          continue
        }
        fresh.push({ key, row, cell })
      }
    }
    if (!board.hasMore) break
    if (page === MAX_RANK_PAGES) truncated = true
  }
  if (truncated) {
    await note(hinina, 'warn', `balloon: board truncated at ${MAX_RANK_PAGES} pages`)
  }

  // Earlier rounds' pending claims retry first (their dispatch never
  // confirmed), then this round's newly claimed first-ACs.
  const retries = Object.entries(records).filter(([, record]) => record.state === 'pending')
  if (retries.length === 0 && fresh.length === 0) {
    await hinina.log.debug(
      `balloon: ${seenTeams.size} teams, ${pages} page(s), ${Object.keys(records).length} known; nothing to do`,
    )
    return
  }

  // Claim before any dispatch leaves the host: pending-on-disk first, so a
  // crash between claim and dispatch retries instead of forgetting. CAS only
  // guarantees the local single-process transaction; the remote idempotency
  // key is what makes redelivery safe on the receiving side.
  let current = records
  const claims: Array<[string, BalloonRecord]> = []
  if (fresh.length > 0) {
    const next: RecordMap = { ...records }
    for (const candidate of fresh) {
      const record = pendingRecord(
        contest,
        problemOf.get(candidate.cell.displayId),
        candidate.row,
        candidate.cell,
        now,
      )
      next[candidate.key] = record
      claims.push([candidate.key, record])
    }
    if (!(await hinina.storage.compareAndSet(recordsKey, stored, next))) {
      await hinina.log.warn('balloon: storage claim lost; round aborted')
      return
    }
    current = next
  }

  const sentKeys: string[] = []
  let failures = 0
  let dead = false
  for (const [key, record] of [...retries, ...claims]) {
    const outcome = await dispatch(hinina, context, settings, contest.contestId, key, record)
    if (outcome === 'dead') {
      dead = true
      break
    }
    if (outcome === 'sent') sentKeys.push(key)
    else failures += 1
  }

  if (sentKeys.length > 0) {
    const next: RecordMap = { ...current }
    for (const key of sentKeys) {
      const record = next[key]
      if (record !== undefined) next[key] = { ...record, state: 'sent', updatedAt: now }
    }
    await hinina.storage.set(recordsKey, next)
  }

  await hinina.log.info(
    `balloon: round done — ${seenTeams.size} teams, ${pages} page(s), ` +
      `dispatched ${sentKeys.length}, failed ${failures}, seal-skipped ${sealSkipped}, ` +
      `known ${Object.keys(current).length}`,
  )
  if (dead) {
    // The scope died mid-queue (or the HTTP budget ran out): everything not
    // marked sent stays pending and is retried on the next Resync.
    await note(hinina, 'info', 'balloon: round stopped mid-queue; remaining balloons stay pending')
  }
}

/** Refresh the persisted seal window; returns the effective memory. */
async function updateSealMemory(
  hinina: Hinina,
  contest: PluginContest,
  now: number,
): Promise<SealWindow | null> {
  const sealKey = SEAL_KEY_PREFIX + contest.contestId
  const seal = readSeal(await hinina.storage.get(sealKey))
  if (contest.sealed) {
    const since = contest.sealedSince ?? now
    if (seal === null || seal.since !== since || seal.revealedAt !== null) {
      const next: SealWindow = { since, revealedAt: null }
      await hinina.storage.set(sealKey, next)
      return next
    }
    return seal
  }
  if (seal !== null && seal.revealedAt === null) {
    const next: SealWindow = { since: seal.since, revealedAt: now }
    await hinina.storage.set(sealKey, next)
    return next
  }
  return seal
}

/**
 * True when the AC happened inside a seal window this plugin observed. Such
 * first-ACs were invisible while sealed and must not be replayed on reveal.
 */
function isSealWindowAc(seal: SealWindow | null, contest: PluginContest, acTime: number | null): boolean {
  if (seal === null || acTime === null) return false
  const at = contest.startsAt + acTime * 1000
  return at >= seal.since && (seal.revealedAt === null || at < seal.revealedAt)
}

function pendingRecord(
  contest: PluginContest,
  problem: PluginContestProblem | undefined,
  row: PluginRankRow,
  cell: PluginRankCell,
  now: number,
): BalloonRecord {
  return {
    state: 'pending',
    displayId: cell.displayId,
    problemId: problem?.problemId ?? '',
    color: problem?.color ?? '',
    teamKey: row.teamKey,
    displayName: row.displayName,
    school: row.school,
    rank: row.rank,
    acTime: cell.acTime,
    updatedAt: now,
  }
}

type DispatchOutcome = 'sent' | 'failed' | 'dead'

async function dispatch(
  hinina: Hinina,
  context: string,
  settings: BalloonOptions,
  contestId: string,
  key: string,
  record: BalloonRecord,
): Promise<DispatchOutcome> {
  try {
    const response = await hinina.http.request(context, {
      method: 'POST',
      url: settings.endpoint,
      headers: { 'content-type': 'application/json' },
      body: {
        // Stable across retries and restarts: the receiver deduplicates on it,
        // which is what makes the local pending→sent flow safe to replay.
        idempotencyKey: `${contestId}:${key}`,
        contestId,
        displayId: record.displayId,
        problemId: record.problemId,
        color: record.color,
        teamKey: record.teamKey,
        displayName: record.displayName,
        school: record.school,
        rank: record.rank,
        acTime: record.acTime,
      },
      timeoutMs: null,
    })
    if (response.ok) return 'sent'
    await note(hinina, 'warn', `balloon: ${record.displayId} dispatch rejected (HTTP ${response.status}); will retry`)
    return 'failed'
  } catch (error) {
    const code = errorCode(error)
    if (code === 'context_expired' || code === 'limit_exceeded') return 'dead'
    await note(hinina, 'warn', `balloon: ${record.displayId} dispatch failed (${code}); will retry`)
    return 'failed'
  }
}

/** Logging is observability, not control flow: a rejected line must not abort the round. */
async function note(hinina: Hinina, level: PluginLogLevel, message: string): Promise<void> {
  try {
    if (level === 'debug') await hinina.log.debug(message)
    else if (level === 'info') await hinina.log.info(message)
    else if (level === 'warn') await hinina.log.warn(message)
    else await hinina.log.error(message)
  } catch {
    // The 30/min outlet limit rejects excess lines; the round continues.
  }
}

function errorCode(error: unknown): string {
  if (error !== null && typeof error === 'object') {
    const code = (error as { code?: unknown }).code
    if (typeof code === 'string') return code
  }
  return 'script_error'
}

function readSeal(stored: PluginStorageValue): SealWindow | null {
  if (stored.state !== 'value') return null
  const value = stored.value
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return null
  const record = value as Record<string, JsonValue>
  const since = record.since
  const revealedAt = record.revealedAt
  if (typeof since !== 'number') return null
  if (revealedAt !== null && revealedAt !== undefined && typeof revealedAt !== 'number') return null
  return { since, revealedAt: revealedAt ?? null }
}

function readRecordMap(stored: PluginStorageValue): RecordMap {
  if (stored.state !== 'value') return {}
  const value = stored.value
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return {}
  const records: RecordMap = {}
  for (const [key, entry] of Object.entries(value as Record<string, JsonValue>)) {
    if (isBalloonRecord(entry)) records[key] = entry
  }
  return records
}

function isBalloonRecord(value: JsonValue): value is BalloonRecord {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return false
  const record = value as Record<string, JsonValue>
  return (
    (record.state === 'pending' || record.state === 'sent') &&
    typeof record.displayId === 'string' &&
    typeof record.problemId === 'string' &&
    typeof record.color === 'string' &&
    typeof record.teamKey === 'string' &&
    typeof record.displayName === 'string' &&
    typeof record.school === 'string' &&
    typeof record.rank === 'number' &&
    (record.acTime === null || typeof record.acTime === 'number') &&
    typeof record.updatedAt === 'number'
  )
}
