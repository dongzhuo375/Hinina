import { pluginBridge } from '@/bridge/plugin.bridge'
import type { PluginNotification } from '@/types/plugin'

export const pluginService = {
  list: () => pluginBridge.list(),
  start: (id: string) => pluginBridge.start(id),
  stop: (id: string) => pluginBridge.stop(id),
  reload: (id: string) => pluginBridge.reload(id),
  resync: (id: string) => pluginBridge.resync(id),
  onNotification: (handler: (notification: PluginNotification) => void) =>
    pluginBridge.onNotification(handler),
}
