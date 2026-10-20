export type PluginState = 'starting' | 'running' | 'stopped' | 'failed'
export interface PluginStatus {
  id: string
  state: PluginState
  error: { code: string; permission?: string } | null
}
export interface PluginNotification {
  pluginId: string
  level: 'info' | 'success' | 'warn' | 'error'
  message: string
}
