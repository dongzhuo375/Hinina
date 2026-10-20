// Compile-only SDK contract checks; no plugin is executed.
import type { Hinina, PluginSubmissionQuery } from './hinina'
declare const hinina: Hinina

hinina.events.on('submission.judged', async envelope => {
  const status: string = envelope.event.status
  void status
  // @ts-expect-error Source code is not part of a plugin event.
  envelope.event.sourceCode
})
hinina.events.on('resync.required', envelope => {
  const lost: number = envelope.event.lost
  void lost
})
// @ts-expect-error Internal credential events cannot be subscribed to.
hinina.events.on('auth.token_rotated', () => {})

const query: PluginSubmissionQuery = { contestId: 'c1', page: 1, pageSize: 50 }
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
