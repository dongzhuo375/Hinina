import { defineStore } from 'pinia'
import { ref } from 'vue'
import { pluginService } from '@/services/plugin.service'
import type { ManagedPluginNotification, NotificationSubscription } from '@/services/plugin.service'
import type { PluginStatus } from '@/types/plugin'

export const usePluginStore = defineStore('plugin', () => {
  const plugins = ref<PluginStatus[]>([])
  const notifications = ref<ManagedPluginNotification[]>([])
  let subscription: NotificationSubscription | undefined
  // Monotonic token: an await that resolves after a newer subscribe must not become active.
  let generation = 0
  async function list() { plugins.value = await pluginService.list(); return plugins.value }
  async function start(id: string) { await pluginService.start(id); return list() }
  async function stop(id: string) { await pluginService.stop(id); return list() }
  async function reload(id: string) { await pluginService.reload(id); return list() }
  async function resync(id: string) { await pluginService.resync(id) }
  function applyDismiss(id: number) { notifications.value = notifications.value.filter(item => item.id !== id) }
  function dismiss(id: number) { subscription?.dismiss(id) }
  async function subscribe() {
    const current = ++generation
    const previous = subscription
    subscription = undefined
    previous?.()
    const next = await pluginService.subscribeNotifications(
      notification => { notifications.value = [...notifications.value, notification] },
      applyDismiss,
    )
    if (current !== generation) {
      // A newer subscribe superseded this one while it was awaiting: dispose the late
      // listener so it can never enqueue notifications or hijack `dismiss`.
      next()
      return next
    }
    subscription = next
    return Object.assign(() => {
      if (subscription !== next) return
      generation++
      subscription = undefined
      next()
    }, { dismiss: next.dismiss })
  }
  return { plugins, notifications, list, start, stop, reload, resync, subscribe, dismiss }
})
