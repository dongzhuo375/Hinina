import { listen } from '@tauri-apps/api/event'
import { ipcInvoke } from '@/bridge'
import type { PluginNotification, PluginStatus } from '@/types/plugin'

export const pluginBridge = {
  list: () => ipcInvoke<PluginStatus[]>('plugin_list'),
  start: (pluginId: string) => ipcInvoke<void>('plugin_start', { pluginId }),
  stop: (pluginId: string) => ipcInvoke<void>('plugin_stop', { pluginId }),
  reload: (pluginId: string) => ipcInvoke<void>('plugin_reload', { pluginId }),
  resync: (pluginId: string) => ipcInvoke<void>('plugin_resync', { pluginId }),
  /// 订阅插件通知。bridge 层只校验载荷**形状**（pluginId/message/level 均为 string，
  /// 畸形整事件丢弃——与 announcement/workspace 事件桥同一口径，越界数据不进 store）；
  /// 长度上限/级别白名单等域规则由 plugin.service 的 validNotification 兜底
  onNotification: (handler: (notification: PluginNotification) => void) =>
    listen<PluginNotification>('plugin-notification', (event) => {
      const payload = event.payload
      if (
        !payload || typeof payload !== 'object'
        || typeof payload.pluginId !== 'string'
        || typeof payload.message !== 'string'
        || typeof payload.level !== 'string'
      ) return
      handler(payload)
    }),
}
