/**
 * Plugin API 2 for the embedded QuickJS host. Bundle TypeScript offline to one ES module.
 * Every Promise-returning host API MUST be awaited, or returned/chained into the
 * Promise returned by activate/an event handler. Await Promise.all for parallel calls.
 * Fire-and-forget calls (void promise, unreturned .then, detached async work) are unsupported:
 * after activate/handler settles there is no guarantee they execute or report rejection.
 * events.on and its unsubscribe function are synchronous and need no await.
 */
export type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue }
export type PluginPermission = 'events.subscribe' | 'submission.read' | 'contest.read' | 'contest.rank' | 'identity.read' | 'storage.read' | 'storage.write' | 'network.request' | 'notification'
export type PluginQueryContext = string
export type PluginStorageValue = { state: 'missing' } | { state: 'value'; value: JsonValue }
export type PluginLogLevel = 'debug' | 'info' | 'warn' | 'error'
export interface PluginSubmission {
  submissionId: string
  contestId: string
  problemId: string
  problemDisplayId: string
  status: string
  submittedAt: number
}
export interface PluginSubmissionQuery {
  contestId: string
  problemId?: string | null
  cursor?: string | null
  limit: number
}
export interface PluginSubmissionPage {
  records: PluginSubmission[]
  total: number | null
  nextCursor: string | null
}
export type PluginContestType = 'acm' | 'oi'
export interface PluginContest {
  contestId: string
  title: string
  startsAt: number
  endsAt: number
  contestType: PluginContestType
  /** True only inside the seal window (sealRank && sealRankTime <= now). Explanatory
   * only: the real gate is the server withholding pass states while sealed. */
  sealed: boolean
  /** UTC ms; when the seal window STARTED (sealRankTime); non-null only while
   * sealed. The server never publishes a seal end, so there is no sealedUntil. */
  sealedSince: number | null
}
export interface PluginContestProblem {
  /** Real problem id. */
  problemId: string
  /** In-contest display id ("A"). */
  displayId: string
  title: string
  /** Balloon color; Hydro has no such field, may be ''. */
  color: string
}
export interface PluginRankQuery {
  /** 1..100 */
  page: number
  /** 1..200 */
  limit: number
}
export interface PluginRankCell {
  displayId: string
  /** Always false while sealed: the server withholds pass states. */
  solved: boolean
  /** Server "first AC" mark; always false while sealed. */
  firstAc: boolean
  /** Seconds since contest start; null while sealed. */
  acTime: number | null
  errorNum: number
  /** Submissions during the seal window. */
  tryNum: number | null
  /** OI score for this problem. */
  score: number | null
  afterContest: boolean
}
export interface PluginRankRow {
  /** -1 marks a starred (unranked) team. */
  rank: number
  /** Host salted hash; same algorithm as identity.userKey, comparable to your own. */
  teamKey: string
  /** Display name resolved by the host per the contest's rankShowName rule —
   * the same text the app's own board shows (realname/nickname/username, then
   * username, then uid as fallbacks). */
  displayName: string
  school: string
  ac: number
  totalScore: number | null
  /** Key = displayId. */
  cells: Record<string, PluginRankCell>
}
export interface PluginRankPage {
  rows: PluginRankRow[]
  /** Includes server-pinned entries; not the participant count. */
  total: number
  /** The provider's actual page, not the requested one (Hydro always reports 1). */
  page: number
  hasMore: boolean
}
export interface PluginIdentity { ojId: string; userKey: string }
export interface PluginHttpRequest {
  method: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE'
  url: string
  headers?: Record<string, string>
  body?: JsonValue
  timeoutMs?: number | null
}
export interface PluginHttpResponse { ok: boolean; status: number; headers: Record<string, string>; body: string }
export type PluginError =
  | { code: 'permission_denied'; permission: PluginPermission }
  | { code: 'invalid_input' | 'context_expired' | 'unauthenticated' | 'not_found' | 'provider_error' | 'limit_exceeded' | 'timeout' | 'unavailable' | 'unsupported' | 'script_error' }
export interface PluginEventMap {
  'system.oj_switched': { type: 'system.oj_switched'; ojId: string }
  'contest.selected': { type: 'contest.selected'; contestId: string }
  'announcement.changed': { type: 'announcement.changed'; contestId: string; newCount: number }
  'problem.opened': { type: 'problem.opened'; contestId: string; problemId: string }
  'submission.created': { type: 'submission.created'; submissionId: string }
  'submission.judged': { type: 'submission.judged'; submissionId: string; status: string }
  'workspace.saved': { type: 'workspace.saved'; workspaceId: string; revision: number; automatic: boolean }
  'system.config_changed': { type: 'system.config_changed' }
  'system.theme_changed': { type: 'system.theme_changed' }
  'session.logged_in': { type: 'session.logged_in'; ojId: string }
  'session.logged_out': { type: 'session.logged_out'; ojId: string }
  'session.expired': { type: 'session.expired'; ojId: string }
  'resync.required': { type: 'resync.required'; lost: number }
}
export type PluginEventName = keyof PluginEventMap
export type PluginEvent = PluginEventMap[PluginEventName]
export interface PluginEventEnvelope<E extends PluginEvent = PluginEvent> {
  version: 2
  sequence: number
  /** Adaptation time in UTC ms; not a submission timestamp. */
  occurredAt: number
  event: E
}
export interface PluginManifest {
  id: string
  name: string
  version: string
  manifestVersion: 2
  runtime: 'js'
  entry: string
  apiVersion: 2
  description?: string
  author?: string
  minAppVersion?: string | null
  permissions: PluginPermission[]
  events: PluginEventName[]
  network?: { allowOrigins: string[]; maxBodyBytes: number; timeoutMs: number }
  contributes?: []
  configSchema?: Record<string, never>
}

/** @plugin-api
 * @since 2.0
 * @stability stable
 * All calls use host-bound grants and stable Promise rejection objects.
 * Non-JSON arguments reject with invalid_input. The host-call timeout cutoff is
 * one second before the activation/event hard deadline; later calls time out immediately.
 */
export interface Hinina {
  /** @plugin-api
   * @since 2.0
   * @permission none
   * @stability stable
   * @async
   */
  context: {
    /** @plugin-api @since 2.0 @permission none @stability stable @async */
    current(): Promise<PluginQueryContext>
  }
  events: {
    /** @plugin-api
     * @since 2.0
     * @permission events.subscribe
     * @stability stable
     * Handlers execute serially. Returns the listener's unsubscribe function.
     */
    on<K extends PluginEventName>(event: K, handler: (event: PluginEventMap[K], envelope: PluginEventEnvelope<PluginEventMap[K]>) => void | Promise<void>): () => void
  }
  submission: {
    /** @plugin-api
     * @since 2.0
     * @permission submission.read
     * @stability stable
     * @async
     * Scans own history for at most 8 seconds / 100 API pages. An incomplete scan
     * rejects with timeout or limit_exceeded, never null; use list for incremental work.
     */
    get(context: PluginQueryContext, contestId: string, submissionId: string): Promise<PluginSubmission | null>
    /** @plugin-api
     * @since 2.0
     * @permission submission.read
     * @stability stable
     * @async
     */
    list(context: PluginQueryContext, query: PluginSubmissionQuery): Promise<PluginSubmissionPage>
  }
  contest: {
    /** @plugin-api
     * @since 2.0
     * @permission contest.read
     * @stability stable
     * @async
     */
    current(context: PluginQueryContext): Promise<PluginContest | null>
    /** @plugin-api
     * @since 2.0
     * @permission contest.read
     * @stability stable
     * @async
     */
    get(context: PluginQueryContext, contestId: string): Promise<PluginContest | null>
    /** @plugin-api
     * @since 2.0
     * @permission contest.read
     * @stability stable
     * @async
     * Problem list of the context-bound contest (problem id ↔ balloon color).
     * No selected contest rejects with context_expired.
     */
    problems(context: PluginQueryContext): Promise<PluginContestProblem[]>
    /** @plugin-api
     * @since 2.0
     * @permission contest.rank
     * @stability stable
     * @async
     * Full-board rank page. Requires BOTH contest.rank and contest.read; pages are
     * live views (no snapshot) — dedupe across pages by teamKey. limit is a hint:
     * Hydro returns the whole board in one page (rows may exceed limit, still
     * bounded by 200 rows / 6,000 total cells); page echoes the provider's page.
     * Rank calls have a dedicated 6/min per-activation budget.
     */
    rank(context: PluginQueryContext, query: PluginRankQuery): Promise<PluginRankPage>
  }
  identity: {
    /** @plugin-api
     * @since 2.0
     * @permission identity.read
     * @stability stable
     * @async
     */
    current(context: PluginQueryContext): Promise<PluginIdentity | null>
  }
  storage: {
    /** @plugin-api
     * @since 2.0
     * @permission storage.read
     * @stability stable
     * @async
     */
    get(key: string): Promise<PluginStorageValue>
    /** @plugin-api
     * @since 2.0
     * @permission storage.write
     * @stability stable
     * @async
     */
    set(key: string, value: JsonValue): Promise<void>
    /** @plugin-api
     * @since 2.0
     * @permission storage.write
     * @stability stable
     * @async
     */
    delete(key: string): Promise<void>
    /** @plugin-api
     * @since 2.0
     * @permission storage.read storage.write
     * @stability stable
     * @async
     */
    compareAndSet(key: string, expected: PluginStorageValue, value: JsonValue): Promise<boolean>
  }
  http: {
    /** @plugin-api
     * @since 2.0
     * @permission network.request
     * @stability stable
     * @async
     */
    request(context: PluginQueryContext, request: PluginHttpRequest): Promise<PluginHttpResponse>
  }
  notification: {
    /** @plugin-api
     * @since 2.0
     * @permission notification
     * @stability stable
     * @async
     */
    info(message: string): Promise<void>
    /** @plugin-api
     * @since 2.0
     * @permission notification
     * @stability stable
     * @async
     */
    success(message: string): Promise<void>
    /** @plugin-api
     * @since 2.0
     * @permission notification
     * @stability stable
     * @async
     */
    warn(message: string): Promise<void>
    /** @plugin-api
     * @since 2.0
     * @permission notification
     * @stability stable
     * @async
     */
    error(message: string): Promise<void>
  }
  log: {
    /** @plugin-api
     * @since 2.0
     * @permission none
     * @stability stable
     * @async
     * Host log outlet: no permission, no host data read. One message is at most
     * 1024 UTF-8 bytes; 30 written lines per minute per activation — calls the
     * host level filter drops succeed without consuming the budget. Messages
     * are plain strings — JSON.stringify objects yourself; there is no console.
     */
    debug(message: string): Promise<void>
    /** @plugin-api
     * @since 2.0
     * @permission none
     * @stability stable
     * @async
     * Host log outlet: no permission, no host data read. One message is at most
     * 1024 UTF-8 bytes; 30 written lines per minute per activation — calls the
     * host level filter drops succeed without consuming the budget. Messages
     * are plain strings — JSON.stringify objects yourself; there is no console.
     */
    info(message: string): Promise<void>
    /** @plugin-api
     * @since 2.0
     * @permission none
     * @stability stable
     * @async
     * Host log outlet: no permission, no host data read. One message is at most
     * 1024 UTF-8 bytes; 30 written lines per minute per activation — calls the
     * host level filter drops succeed without consuming the budget. Messages
     * are plain strings — JSON.stringify objects yourself; there is no console.
     */
    warn(message: string): Promise<void>
    /** @plugin-api
     * @since 2.0
     * @permission none
     * @stability stable
     * @async
     * Host log outlet: no permission, no host data read. One message is at most
     * 1024 UTF-8 bytes; 30 written lines per minute per activation — calls the
     * host level filter drops succeed without consuming the budget. Messages
     * are plain strings — JSON.stringify objects yourself; there is no console.
     */
    error(message: string): Promise<void>
  }
}

/** @plugin-entry @runtime js @api-version 2 */
export type PluginActivate = (hinina: Hinina, config: Readonly<JsonValue>) => Promise<void>
