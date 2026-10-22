import { defineStore } from 'pinia'
import { ref } from 'vue'
import { pluginService } from '@/services/plugin.service'
import type { ManagedPluginNotification } from '@/services/plugin.service'
import type { PluginStatus } from '@/types/plugin'

export const usePluginStore = defineStore('plugin', () => {
  const plugins = ref<PluginStatus[]>([])
  const notifications = ref<ManagedPluginNotification[]>([])
  async function list() { plugins.value = await pluginService.list(); return plugins.value }
  async function start(id: string) { await pluginService.start(id); return list() }
  async function stop(id: string) { await pluginService.stop(id); return list() }
  async function reload(id: string) { await pluginService.reload(id); return list() }
  async function resync(id: string) { await pluginService.resync(id) }
  function applyDismiss(id: number) { notifications.value = notifications.value.filter(item => item.id !== id) }
  function dismiss(id: number) { pluginService.dismissNotification(id) }
  // Subscription lifecycle (single active listener, superseding, late arrivals) is Service
  // policy; the Store only applies the add/dismiss events to reactive state.
  function subscribe() {
    return pluginService.subscribeNotifications(
      notification => { notifications.value = [...notifications.value, notification] },
      applyDismiss,
    )
  }
  return { plugins, notifications, list, start, stop, reload, resync, subscribe, dismiss }
})
