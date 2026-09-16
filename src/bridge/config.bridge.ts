import type { AppConfig } from '@/types/config'
import { ipcInvoke } from '@/bridge'

/** 获取应用配置 */
export async function getConfig(): Promise<AppConfig> {
  return ipcInvoke<AppConfig>('get_config')
}

/** 整体替换应用配置（Rust 端持久化到 config.json 并更新内存值） */
export async function updateConfig(config: AppConfig): Promise<void> {
  return ipcInvoke<void>('update_config', { config })
}

/** 触发后端从磁盘重载配置（发布 ConfigReloaded 事件） */
export async function reloadConfig(): Promise<void> {
  return ipcInvoke<void>('reload_config')
}
