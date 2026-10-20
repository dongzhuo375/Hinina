/** Plugin API 2 for the embedded QuickJS host. Bundle TypeScript offline to one ES module. */
export type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue }
export type PluginPermission = 'events.subscribe' | 'submission.read' | 'contest.read' | 'identity.read' | 'storage.read' | 'storage.write' | 'network.request' | 'notification'
export type PluginQueryContext = string
export type PluginStorageValue = { state: 'missing' } | { state: 'value'; value: JsonValue }
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
export interface PluginContest { contestId: string; title: string; startsAt: number; endsAt: number }
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
}

/** @plugin-entry @runtime js @api-version 2 */
export type PluginActivate = (hinina: Hinina, config: Readonly<JsonValue>) => Promise<void>
