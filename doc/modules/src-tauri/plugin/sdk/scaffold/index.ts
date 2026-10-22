import type { Hinina, JsonValue } from '../hinina'

/**
 * @plugin-entry
 * @plugin-id com.example.development
 * @runtime js
 * @api-version 2
 */
export default async function activate(hinina: Hinina, _config: Readonly<JsonValue>): Promise<void> {
  // Await every host API Promise in activate/handlers; detached work is unsupported.
  // events.on itself is synchronous; its async handler must await its own host calls.
  /**
   * @plugin-handler
   * @event resync.required
   * @idempotent true
   * @resync-safe true
   */
  hinina.events.on('resync.required', async () => {
    // Replace this development probe with bounded reconciliation of current state.
    const previous = await hinina.storage.get('development.ready')
    if (previous.state === 'missing') {
      await hinina.storage.set('development.ready', { initialized: true })
    }
  })
}
