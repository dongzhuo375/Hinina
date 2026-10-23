// Compile-only SDK contract checks; no plugin is executed.
import type { Hinina, PluginLogLevel, PluginSubmissionQuery } from './hinina'
declare const hinina: Hinina

hinina.events.on('submission.judged', async event => {
  const status: string = event.status
  void status
  // @ts-expect-error Source code is not part of a plugin event.
  event.sourceCode
})
hinina.events.on('resync.required', event => {
  const lost: number = event.lost
  void lost
})
// @ts-expect-error Internal credential events cannot be subscribed to.
hinina.events.on('auth.token_rotated', () => {})

const query: PluginSubmissionQuery = { contestId: 'c1', cursor: null, limit: 50 }
// @ts-expect-error The caller cannot request other users' submissions.
query.onlyMine = false
void query

async function checkStorageNull(): Promise<void> {
  const result = await hinina.storage.get('marker')
  if (result.state === 'value') {
    await hinina.storage.compareAndSet('marker', result, null)
  }
}
void checkStorageNull

async function checkLog(): Promise<void> {
  await hinina.log.info('x')
  // @ts-expect-error Log messages must be strings; stringify objects yourself.
  await hinina.log.info(42)
  const level: PluginLogLevel = 'warn'
  void level
}
void checkLog
