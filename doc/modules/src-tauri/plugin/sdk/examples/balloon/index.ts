import type {
  Hinina,
  JsonValue,
  PluginContest,
  PluginContestProblem,
  PluginLogLevel,
  PluginRankCell,
  PluginRankPage,
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
 *   contest.problems → contest.rank page by page (dedupe rows by teamKey;
 *   rank has a hard 6 queries/minute per-activation budget, so paging shares
 *   one 60s window across rounds and walks a cursor across the board — a
 *   spent window or a transient limit_exceeded ends paging, never the round,
 *   and a page rejected twice in a row is oversized and skipped so the
 *   cursor can never pin) → for every first-AC cell not yet sent: claim
 *   `pending` via CAS → POST to the configured endpoint with an idempotency
 *   key → mark `sent` on disk after every single success (the 30s handler
 *   deadline can kill the round at any moment, so buffering sent markers in
 *   memory would lose them).
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

/** Contract bound for the paging cursor: wraps back to page 1 past it. */
const MAX_RANK_PAGES = 100
/**
 * Host page bounds the plugin must respect when shaping a rank request: at
 * most 200 rows and 6,000 cells (rows × columns) per page. Rows per page
 * adapt to the problem count so a page stays encodable — a fixed 200 makes
 * a 40-problem board fail every page at 200 × 40 = 8,000 cells.
 */
const RANK_MAX_ROWS = 200
const RANK_MAX_CELLS = 6_000
/**
 * Local rank-query budget with the host limiter's exact GCRA parameters
 * (governor `Quota::per_minute(6)` = burst 6, one token replenished every
 * 10s); keep in sync with the host. Running the same algorithm locally
 * makes the plugin's accounting equivalent to the host's by construction.
 * A fixed 60s window is NOT equivalent: spending near the end of one window
 * lets the next window start seconds later with a "fresh" budget the host
 * never refilled (the counterexample schedule is locked by a test).
 */
const RANK_QUERIES_PER_MINUTE = 6
/**
 * Consecutive 60s windows (anchored at rankPaging.since) a page must be
 * rejected in before it is skipped. At the recommended ≥30s resync interval
 * this is roughly two minutes of sustained failure: one- or two-window
 * quota/shared-budget hiccups never sacrifice a page, while a truly
 * oversized page (or a lasting drought) is still skipped instead of pinning
 * the cursor forever.
 */
const SKIP_AFTER_WINDOWS = 3
const RECORDS_KEY_PREFIX = 'balloon.records:'
const SEAL_KEY_PREFIX = 'balloon.seal:'
/** Cannot collide with a host-minted query-context handle. */
const PROBE_CONTEXT = '__balloon_capability_probe__'

/**
 * Rank paging budget and cursor, shared across rounds within one
 * activation. Deliberately module state, NOT storage: the host budget
 * resets with each activation (start/reload/restart) exactly like this
 * module instance, so a persisted count would under-use a fresh budget
 * after a reload.
 *
 * `tokens`/`last` mirror the host's GCRA limiter with the same parameters
 * (burst 6, one token per 10s) on the envelope clock — the envelope's
 * occurredAt IS the host wall clock at adaptation time (event_adapter
 * wrap()), so the only skew is the queueing delay until this handler runs;
 * that delay can only make the local bucket LAG the host's (conservative
 * direction). Everything the local bucket cannot see — other plugins on
 * the shared provider budget, content-rejected pages, delay spikes —
 * surfaces as a transient limit_exceeded that the retry/skip path in
 * pageRank() absorbs.
 *
 * `since` is NOT part of the budget: it anchors the 60s bucket used only
 * for the consecutive-window skip counting (`retryWindows`), and the cursor
 * walks the board across rounds, wrapping at the end — a board deeper than
 * one burst still rotates fully into coverage over time instead of being
 * cut off at a fixed page.
 */
const rankPaging = {
  contest: '',
  /** 60s anchor for the skip-window counting (not the budget). */
  since: 0,
  /** Local GCRA accounting; starts full like a fresh host limiter. */
  tokens: RANK_QUERIES_PER_MINUTE,
  last: 0,
  nextPage: 1,
  retryPage: 0,
  retryWindows: 0,
  retryWindow: 0,
}

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

/**
 * Read and validate `options.endpoint`: an https URL with no whitespace,
 * userinfo, or fragment — the host's send path rejects all three, and a bad
 * value must fail activation instead of failing every dispatch forever. Query
 * strings are allowed; loopback/private targets are the host's address
 * policy, not checked here.
 */
function readOptions(config: Readonly<JsonValue>): BalloonOptions {
  if (config === null || typeof config !== 'object' || Array.isArray(config)) {
    throw Object.freeze({ code: 'invalid_input' })
  }
  const endpoint = (config as Record<string, JsonValue>).endpoint
  if (typeof endpoint !== 'string' || !/^https:\/\/[^\s/@#]+(\/[^\s#]*)?$/.test(endpoint)) {
    throw Object.freeze({ code: 'invalid_input' })
  }
  return { endpoint }
}

/**
 * Fail fast at activation when an approved permission is missing: a plugin
 * that can never dispatch should land in `failed` with the missing permission
 * instead of silently no-op-ing every round.
 *
 * The host maps ANY activate() rejection to `failed`/`script_error` without
 * reading the thrown code, so every probe logs the precise cause to the host
 * log (via the permission-free log outlet) before rethrowing — the log line
 * is the operator's diagnostic, the state's error code is not.
 *
 * Permission probes are side-effect free: the dummy context fails the scope
 * check before any provider call, and the storage probe compares against a
 * value that cannot exist (no write happens). Origin approval and URL
 * acceptance live behind the scope check (the host enforces both only in its
 * send path), so they need a real context — see `probeEndpoint`.
 */
async function probeCapabilities(hinina: Hinina, settings: BalloonOptions): Promise<void> {
  // contest.rank checks contest.rank THEN contest.read, then the scope check.
  await probeCall(hinina, 'contest.rank probe', () =>
    expectContextExpired(hinina.contest.rank(PROBE_CONTEXT, { page: 1, limit: 1 })),
  )
  // compareAndSet checks storage.read + storage.write; a fresh key never
  // matches the expected value, so the comparison fails without writing.
  await probeCall(hinina, 'storage probe', async () => {
    if (await hinina.storage.compareAndSet('balloon.probe', { state: 'value', value: null }, null)) {
      await hinina.storage.delete('balloon.probe')
    }
  })
  // http.request checks network.request, then the shape, then the scope check.
  await probeCall(hinina, 'http.request probe', () =>
    expectContextExpired(
      hinina.http.request(PROBE_CONTEXT, { method: 'GET', url: settings.endpoint, timeoutMs: null }),
    ),
  )
  await probeEndpoint(hinina, settings)
}

/** Log the precise failure for the operator, then rethrow to fail activation. */
async function probeCall(hinina: Hinina, label: string, probe: () => Promise<void>): Promise<void> {
  try {
    await probe()
  } catch (error) {
    await note(hinina, 'error', `balloon: activation failed at ${label}: ${describeError(error)}`)
    throw error
  }
}

/**
 * One GET against the endpoint proves the origin is approved in BOTH
 * manifest.json network.allowOrigins and plugins.json network, and that the
 * host accepts the URL — the exact checks the dummy-context probes cannot
 * reach. Only configuration errors fail activation; without a query context
 * (typical when the app starts before login) or on a network-level failure,
 * the first dispatch performs the same checks, and config-class errors there
 * stop the queue instead of retrying forever.
 */
async function probeEndpoint(hinina: Hinina, settings: BalloonOptions): Promise<void> {
  let context: string
  try {
    context = await hinina.context.current()
  } catch {
    await note(
      hinina,
      'info',
      'balloon: no query context at activation; endpoint approval is checked on first dispatch',
    )
    return
  }
  try {
    await hinina.http.request(context, { method: 'GET', url: settings.endpoint, timeoutMs: null })
  } catch (error) {
    const code = errorCode(error)
    if (code === 'permission_denied' || code === 'invalid_input') {
      await note(
        hinina,
        'error',
        `balloon: activation failed at endpoint probe (${code}): the endpoint origin is not approved ` +
          `in manifest.json network.allowOrigins AND plugins.json network, or the host rejects the URL`,
      )
      throw error
    }
    await note(
      hinina,
      'info',
      `balloon: endpoint probe inconclusive (${code}); approval is checked on first dispatch`,
    )
    return
  }
  // Any response — including a non-2xx — proves approval and URL acceptance.
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
      // The dispatch path already treats its own permission_denied as dead;
      // reaching here means a query permission was revoked without a reload.
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
    await note(hinina, 'debug', 'balloon: no contest selected; round skipped')
    return
  }

  const seal = await updateSealMemory(hinina, contest, now)
  if (contest.sealed) {
    // A sealed board withholds pass states: "not solved" is indistinguishable
    // from "solved during the seal". Never read the board, never dispatch —
    // otherwise the reveal would replay a batch of stale balloons.
    await note(hinina, 'info', `balloon: sealed since ${seal?.since ?? now}; round skipped`)
    return
  }

  const problems = await hinina.contest.problems(context)

  const recordsKey = RECORDS_KEY_PREFIX + contest.contestId
  const stored = await hinina.storage.get(recordsKey)
  const records = readRecordMap(stored)

  const scan = await pageRank(hinina, context, contest, seal, records, problems, now)
  if (scan.unknownIds.size > 0) {
    await note(
      hinina,
      'warn',
      `balloon: board columns missing from the problem list: ${[...scan.unknownIds].sort().join(', ')}; skipped this round`,
    )
  }

  // Earlier rounds' pending claims retry first (their dispatch never
  // confirmed), then this round's newly claimed first-ACs.
  const retries = Object.entries(records).filter(([, record]) => record.state === 'pending')
  if (retries.length === 0 && scan.fresh.length === 0) {
    await note(
      hinina,
      'debug',
      `balloon: ${scan.teams} teams, ${scan.pages} page(s)` +
        (scan.pages === 0 && scan.budgetEmpty ? ', rank budget exhausted' : '') +
        `, ${Object.keys(records).length} known; nothing to do`,
    )
    return
  }

  if (seal === null && scan.fresh.length > 0 && Object.keys(records).length === 0) {
    // Seal memory exists only if this plugin observed the board sealed.
    // Started after the reveal (or reloaded mid-contest), it cannot tell a
    // pre-start seal window from an ordinary cold start, so the no-backfill
    // rule cannot apply — disclose that instead of dispatching silently.
    await note(
      hinina,
      'info',
      'balloon: no seal memory for this contest; if it was sealed before this plugin started, those first-ACs dispatch as fresh',
    )
  }

  // Claim before any dispatch leaves the host: pending-on-disk first, so a
  // crash between claim and dispatch retries instead of forgetting. CAS only
  // guarantees the local single-process transaction; the remote idempotency
  // key is what makes redelivery safe on the receiving side.
  let current = records
  const claims: Array<[string, BalloonRecord]> = []
  if (scan.fresh.length > 0) {
    const next: RecordMap = { ...records }
    for (const candidate of scan.fresh) {
      const record = pendingRecord(candidate.problem, candidate.row, candidate.cell, now)
      next[candidate.key] = record
      claims.push([candidate.key, record])
    }
    if (!(await hinina.storage.compareAndSet(recordsKey, stored, next))) {
      await note(hinina, 'warn', 'balloon: storage claim lost; round aborted')
      return
    }
    current = next
  }

  const outcome = await dispatchQueue(
    hinina,
    context,
    settings,
    contest.contestId,
    recordsKey,
    current,
    [...retries, ...claims],
    now,
  )

  await note(
    hinina,
    'info',
    `balloon: round done — ${scan.teams} teams, ${scan.pages} page(s)` +
      (scan.skippedPages > 0 ? `, page-skipped ${scan.skippedPages}` : '') +
      `, dispatched ${outcome.dispatched}, failed ${outcome.failures}, seal-skipped ${scan.sealSkipped}` +
      (scan.unknownIds.size > 0 ? `, unknown ${scan.unknownIds.size}` : '') +
      `, known ${Object.keys(outcome.current).length}`,
  )
  if (outcome.dead) {
    // The scope died mid-queue, a budget ran out, or the 30s handler
    // deadline is close: everything not marked sent stays pending and is
    // retried on the next Resync.
    await note(hinina, 'info', 'balloon: round stopped mid-queue; remaining balloons stay pending')
  }
}

/** Everything the paging phase produced in one round. */
type BoardScan = {
  /** Unclaimed first-AC cells, in board order. */
  fresh: Array<{ key: string; problem: PluginContestProblem; row: PluginRankRow; cell: PluginRankCell }>
  /** Distinct teams seen across the fetched pages (for the round summary). */
  teams: number
  /** Board columns missing from the problem list. */
  unknownIds: Set<string>
  sealSkipped: number
  pages: number
  skippedPages: number
  /** True when the local GCRA bucket had no whole token before this round. */
  budgetEmpty: boolean
}

/**
 * Walk the rank board inside the local GCRA budget and collect the round's
 * fresh first-AC cells. Paging never aborts the round: an empty bucket, a
 * transient limit_exceeded, or a skipped page all just end paging —
 * whatever was already collected is still claimed and dispatched.
 */
async function pageRank(
  hinina: Hinina,
  context: string,
  contest: PluginContest,
  seal: SealWindow | null,
  records: RecordMap,
  problems: PluginContestProblem[],
  now: number,
): Promise<BoardScan> {
  // Live view, no snapshot across pages: dedupe rows by teamKey within the
  // round; cross-round dedupe is the records map (key = teamKey:displayId).
  const problemOf = new Map(
    problems.map((problem): [string, PluginContestProblem] => [problem.displayId, problem]),
  )
  if (rankPaging.contest !== contest.contestId) {
    rankPaging.contest = contest.contestId
    rankPaging.nextPage = 1
    rankPaging.retryPage = 0
    rankPaging.retryWindows = 0
  }
  // Local GCRA refill on the envelope clock (see rankPaging). The guard
  // keeps a regressing envelope time from over-refilling.
  if (now > rankPaging.last) {
    const period = 60_000 / RANK_QUERIES_PER_MINUTE
    rankPaging.tokens = Math.min(
      RANK_QUERIES_PER_MINUTE,
      rankPaging.tokens + (now - rankPaging.last) / period,
    )
    rankPaging.last = now
  }
  // 60s anchor for the skip-window counting only (see rankPaging).
  if (now - rankPaging.since >= 60_000 || now < rankPaging.since) {
    rankPaging.since = now
  }
  // Rows per page adapt to the problem count (+1 spare column for board /
  // problem-list display-id drift) so the page stays under the host's cell
  // bound; the row bound caps it. problems.length, not the Map size: a
  // duplicate displayId must shrink the page, not slip past the bound.
  const pageRows = Math.max(
    1,
    Math.min(RANK_MAX_ROWS, Math.floor(RANK_MAX_CELLS / (problems.length + 1))),
  )
  let budget = Math.floor(rankPaging.tokens)
  const budgetEmpty = budget === 0
  const seenTeams = new Set<string>()
  const fresh: BoardScan['fresh'] = []
  const unknownIds = new Set<string>()
  let sealSkipped = 0
  let pages = 0
  let skippedPages = 0
  while (budget > 0) {
    let board: PluginRankPage
    try {
      board = await hinina.contest.rank(context, { page: rankPaging.nextPage, limit: pageRows })
    } catch (error) {
      if (errorCode(error) !== 'limit_exceeded') throw error
      // limit_exceeded covers the per-minute quota, the shared provider
      // budget, AND pages the host refuses on content grounds (>200 rows or
      // >6,000 cells — Hydro returns its whole board as one page regardless
      // of the requested limit). The cases are indistinguishable, so the
      // page is retried until it has been rejected in SKIP_AFTER_WINDOWS
      // consecutive windows: transient budget hiccups heal within a window
      // or two, while anything lasting would pin the cursor forever. Every
      // attempt spends a local token — the host charges for content-rejected
      // pages too, and rate-rejected attempts only make us conservative.
      rankPaging.tokens = Math.max(0, rankPaging.tokens - 1)
      budget -= 1
      if (rankPaging.retryPage !== rankPaging.nextPage) {
        rankPaging.retryPage = rankPaging.nextPage
        rankPaging.retryWindows = 1
        rankPaging.retryWindow = rankPaging.since
      } else if (rankPaging.retryWindow !== rankPaging.since) {
        rankPaging.retryWindows += 1
        rankPaging.retryWindow = rankPaging.since
      }
      if (rankPaging.retryWindows >= SKIP_AFTER_WINDOWS) {
        await note(
          hinina,
          'warn',
          `balloon: rank page ${rankPaging.nextPage} rejected in ${SKIP_AFTER_WINDOWS} consecutive windows ` +
            `(limit_exceeded): the page exceeds the host caps (${RANK_MAX_ROWS} rows / ${RANK_MAX_CELLS} cells) ` +
            `or the budget drought is lasting; skipping it — its teams get no balloons until a later rotation`,
        )
        rankPaging.retryPage = 0
        rankPaging.retryWindows = 0
        rankPaging.nextPage = await nextCursorPage(hinina, rankPaging.nextPage)
        skippedPages += 1
        continue
      }
      await note(
        hinina,
        'warn',
        `balloon: rank page ${rankPaging.nextPage} rejected (limit_exceeded), ` +
          `window ${rankPaging.retryWindows} of ${SKIP_AFTER_WINDOWS}; retrying the same page`,
      )
      break
    }
    rankPaging.retryPage = 0
    rankPaging.retryWindows = 0
    rankPaging.tokens -= 1
    budget -= 1
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
        const problem = problemOf.get(cell.displayId)
        if (problem === undefined) {
          // Board column missing from the problem list (problem added
          // mid-contest, adapter display-id drift): a balloon without a real
          // problem id and color is useless. Leave the cell unclaimed and
          // retry once the problem list catches up.
          unknownIds.add(cell.displayId)
          continue
        }
        fresh.push({ key, problem, row, cell })
      }
    }
    if (board.hasMore) {
      rankPaging.nextPage = await nextCursorPage(hinina, rankPaging.nextPage)
    } else {
      rankPaging.nextPage = 1
      break
    }
  }
  return { fresh, teams: seenTeams.size, unknownIds, sealSkipped, pages, skippedPages, budgetEmpty }
}

/** Result of draining one dispatch queue. */
type QueueOutcome = {
  dispatched: number
  failures: number
  dead: boolean
  /** The record map including this queue's persisted sent markers. */
  current: RecordMap
}

/**
 * Drain the dispatch queue (retries first, then this round's claims),
 * persisting every sent marker the moment it happens: the host kills the
 * whole handler at a 30s hard deadline, and a sent marker still buffered in
 * memory would leave the record pending on disk, re-dispatched by every
 * later round. A dead outcome stops the queue; the rest stays pending and
 * retries on the next Resync.
 */
async function dispatchQueue(
  hinina: Hinina,
  context: string,
  settings: BalloonOptions,
  contestId: string,
  recordsKey: string,
  current: RecordMap,
  queue: Array<[string, BalloonRecord]>,
  now: number,
): Promise<QueueOutcome> {
  let records = current
  let dispatched = 0
  let failures = 0
  let dead = false
  for (const [key, record] of queue) {
    const outcome = await dispatch(hinina, context, settings, contestId, key, record)
    if (outcome === 'dead') {
      dead = true
      break
    }
    if (outcome !== 'sent') {
      failures += 1
      continue
    }
    dispatched += 1
    const next: RecordMap = {
      ...records,
      [key]: { ...records[key], state: 'sent' as const, updatedAt: now },
    }
    try {
      await hinina.storage.set(recordsKey, next)
      records = next
    } catch (error) {
      await note(hinina, 'warn', `balloon: persisting sent failed (${errorCode(error)}); round stopped`)
      dead = true
      break
    }
  }
  return { dispatched, failures, dead, current: records }
}

/** Advance the cursor past `page`, wrapping at the contract bound with a warning. */
async function nextCursorPage(hinina: Hinina, page: number): Promise<number> {
  if (page < MAX_RANK_PAGES) return page + 1
  await note(
    hinina,
    'warn',
    `balloon: board deeper than the ${MAX_RANK_PAGES}-page contract bound; deeper pages are not scanned`,
  )
  return 1
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
  problem: PluginContestProblem,
  row: PluginRankRow,
  cell: PluginRankCell,
  now: number,
): BalloonRecord {
  return {
    state: 'pending',
    displayId: cell.displayId,
    problemId: problem.problemId,
    color: problem.color,
    teamKey: row.teamKey,
    displayName: row.displayName,
    school: row.school,
    rank: row.rank,
    acTime: cell.acTime,
    updatedAt: now,
  }
}

type DispatchOutcome = 'sent' | 'failed' | 'dead'

/**
 * `dead` ends the queue for this round:
 * - context_expired / limit_exceeded: the scope or a budget is gone.
 * - timeout: either this one request outlived the HTTP deadline (default
 *   5s, capped by the manifest) or the 30s handler deadline is close and
 *   every later host call would return timeout too. The plugin cannot tell
 *   the two apart, so it settles for the conservative reading — stop now,
 *   retry next round; a persistently slow endpoint thereby throttles itself
 *   to one balloon per round instead of risking a mid-queue kill.
 * - permission_denied / invalid_input: configuration errors do not fix
 *   themselves by retrying.
 */
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
      // No content-type: the host blacklists that header and sets
      // application/json itself whenever a body is present.
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
    if (code === 'timeout') {
      await note(hinina, 'warn', `balloon: ${record.displayId} dispatch timed out; will retry next resync`)
      return 'dead'
    }
    if (code === 'permission_denied') {
      await note(
        hinina,
        'error',
        `balloon: dispatch rejected by host policy (${describeError(error)}): network.request is not approved, ` +
          `or the endpoint origin is missing from manifest.json network.allowOrigins AND plugins.json network; queue stopped`,
      )
      return 'dead'
    }
    if (code === 'invalid_input') {
      await note(
        hinina,
        'error',
        'balloon: dispatch rejected by host policy (invalid_input): the endpoint URL or request shape is invalid; queue stopped',
      )
      return 'dead'
    }
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

/** Host-rejection shape for log lines: `permission_denied{network.request}` or the bare code. */
function describeError(error: unknown): string {
  if (error !== null && typeof error === 'object') {
    const { code, permission } = error as { code?: unknown; permission?: unknown }
    if (typeof code === 'string') {
      return typeof permission === 'string' ? `${code}{${permission}}` : code
    }
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
