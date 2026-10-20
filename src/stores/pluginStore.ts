import { defineStore } from 'pinia'
import { ref } from 'vue'
import { pluginService } from '@/services/plugin.service'
import type { PluginNotification, PluginStatus } from '@/types/plugin'

export const usePluginStore = defineStore('plugin', () => {
  const plugins = ref<PluginStatus[]>([])
  const notifications = ref<(PluginNotification & { id: number })[]>([])
  let nextId = 0
  async function list() { plugins.value = await pluginService.list(); return plugins.value }
  async function start(id: string) { await pluginService.start(id); return list() }
  async function stop(id: string) { await pluginService.stop(id); return list() }
  async function reload(id: string) { await pluginService.reload(id); return list() }
  async function resync(id: string) { await pluginService.resync(id) }
  function dismiss(id: number) { notifications.value = notifications.value.filter(item => item.id !== id) }
  function subscribe() {
    return pluginService.onNotification(notification => {
      if (!notification || typeof notification.message !== 'string' || notification.message.length > 512 ||
        typeof notification.pluginId !== 'string' || notification.pluginId.length > 128 ||
        !['info', 'success', 'warn', 'error'].includes(notification.level)) return
      const id = ++nextId
      notifications.value = [...notifications.value.slice(-4), { ...notification, id }]
      window.setTimeout(() => dismiss(id), 6000)
    })
  }
  return { plugins, notifications, list, start, stop, reload, resync, subscribe, dismiss }
})
