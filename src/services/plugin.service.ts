import { pluginBridge } from '@/bridge/plugin.bridge'
import type { PluginNotification } from '@/types/plugin'

export type ManagedPluginNotification = PluginNotification & { id: number }
export type NotificationSubscription = (() => void) & { dismiss: (id: number) => void }
let nextNotificationId = 0

const NOTIFICATION_LEVELS = new Set<PluginNotification['level']>(['info', 'success', 'warn', 'error'])
const MAX_NOTIFICATIONS = 5
const NOTIFICATION_LIFETIME_MS = 6000

function validNotification(value: unknown): value is PluginNotification {
  if (!value || typeof value !== 'object') return false
  const notification = value as Partial<PluginNotification>
  return typeof notification.pluginId === 'string' && notification.pluginId.length <= 128
    && typeof notification.message === 'string' && notification.message.length <= 512
    && typeof notification.level === 'string' && NOTIFICATION_LEVELS.has(notification.level as PluginNotification['level'])
}

/** One bridge listener plus its queue policy (validation, 5-item cap, 6s expiry). */
async function openNotificationSubscription(
  onAdd: (notification: ManagedPluginNotification) => void,
  onDismiss: (id: number) => void,
): Promise<NotificationSubscription> {
  let disposed = false
  const visible: number[] = []
  const timers = new Map<number, ReturnType<typeof window.setTimeout>>()
  const dismiss = (id: number) => {
    const index = visible.indexOf(id)
    if (index < 0) return
    visible.splice(index, 1)
    const timer = timers.get(id)
    if (timer !== undefined) window.clearTimeout(timer)
    timers.delete(id)
    onDismiss(id)
  }
  let unlisten: (() => void) | undefined
  const clear = () => {
    disposed = true
    for (const id of [...visible]) dismiss(id)
  }
  try {
    unlisten = await pluginBridge.onNotification(value => {
      if (disposed || !validNotification(value)) return
      const id = ++nextNotificationId
      if (visible.length === MAX_NOTIFICATIONS) dismiss(visible[0])
      visible.push(id)
      timers.set(id, window.setTimeout(() => dismiss(id), NOTIFICATION_LIFETIME_MS))
      onAdd({ ...value, id })
    })
  } catch (error) {
    clear()
    throw error
  }
  return Object.assign(() => {
    if (disposed) return
    try { unlisten?.() } finally { clear() }
  }, { dismiss })
}

// The bridge listener is process-wide, so "which subscription is active" is service state.
// A monotonic generation lets a late-resolving subscribe notice it was superseded.
let activeSubscription: NotificationSubscription | undefined
let subscriptionGeneration = 0

export const pluginService = {
  list: () => pluginBridge.list(),
  start: (id: string) => pluginBridge.start(id),
  stop: (id: string) => pluginBridge.stop(id),
  reload: (id: string) => pluginBridge.reload(id),
  resync: (id: string) => pluginBridge.resync(id),
  /**
   * Subscribe to plugin notifications. Only one subscription is active: a new call first
   * disposes the previous one (releasing its listener and visible items), and a call that
   * resolves after a newer one started is disposed immediately instead of becoming active.
   * The returned disposer only acts while its subscription is still the active one.
   */
  async subscribeNotifications(
    onAdd: (notification: ManagedPluginNotification) => void,
    onDismiss: (id: number) => void,
  ): Promise<NotificationSubscription> {
    const current = ++subscriptionGeneration
    const previous = activeSubscription
    activeSubscription = undefined
    previous?.()
    const next = await openNotificationSubscription(onAdd, onDismiss)
    if (current !== subscriptionGeneration) {
      next()
      return next
    }
    activeSubscription = next
    return Object.assign(() => {
      if (activeSubscription !== next) return
      subscriptionGeneration++
      activeSubscription = undefined
      next()
    }, { dismiss: next.dismiss })
  },
  /** Dismiss one visible notification of the active subscription (no-op otherwise). */
  dismissNotification: (id: number) => activeSubscription?.dismiss(id),
}
