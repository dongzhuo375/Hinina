import { defineStore } from 'pinia'
import { ref } from 'vue'
import { pluginService } from '@/services/plugin.service'
import type { ManagedPluginNotification, NotificationSubscription } from '@/services/plugin.service'
import type { PluginStatus } from '@/types/plugin'

export const usePluginStore = defineStore('plugin', () => {
  const plugins = ref<PluginStatus[]>([])
  const notifications = ref<ManagedPluginNotification[]>([])
  let subscription: NotificationSubscription | undefined
  async function list() { plugins.value = await pluginService.list(); return plugins.value }
  async function start(id: string) { await pluginService.start(id); return list() }
  async function stop(id: string) { await pluginService.stop(id); return list() }
  async function reload(id: string) { await pluginService.reload(id); return list() }
  async function resync(id: string) { await pluginService.resync(id) }
  function applyDismiss(id: number) { notifications.value = notifications.value.filter(item => item.id !== id) }
  function dismiss(id: number) { subscription?.dismiss(id) }
  async function subscribe() {
    subscription?.()
    subscription = await pluginService.subscribeNotifications(
      notification => { notifications.value = [...notifications.value, notification] },
      applyDismiss,
    )
    return subscription
  }
  return { plugins, notifications, list, start, stop, reload, resync, subscribe, dismiss }
})
