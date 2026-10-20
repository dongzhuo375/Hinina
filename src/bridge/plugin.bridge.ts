import { listen } from '@tauri-apps/api/event'
import { ipcInvoke } from '@/bridge'
import type { PluginNotification, PluginStatus } from '@/types/plugin'

export const pluginBridge = {
  list: () => ipcInvoke<PluginStatus[]>('plugin_list'),
  start: (pluginId: string) => ipcInvoke<void>('plugin_start', { pluginId }),
  stop: (pluginId: string) => ipcInvoke<void>('plugin_stop', { pluginId }),
  reload: (pluginId: string) => ipcInvoke<void>('plugin_reload', { pluginId }),
  resync: (pluginId: string) => ipcInvoke<void>('plugin_resync', { pluginId }),
  onNotification: (handler: (notification: PluginNotification) => void) =>
    listen<PluginNotification>('plugin-notification', event => handler(event.payload)),
}
