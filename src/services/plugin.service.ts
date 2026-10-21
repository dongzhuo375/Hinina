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

export const pluginService = {
  list: () => pluginBridge.list(),
  start: (id: string) => pluginBridge.start(id),
  stop: (id: string) => pluginBridge.stop(id),
  reload: (id: string) => pluginBridge.reload(id),
  resync: (id: string) => pluginBridge.resync(id),
  async subscribeNotifications(
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
  },
}
