import type { AppConfig } from '@/types/config'
import { ipcInvoke } from '@/bridge'

/** 获取应用配置 */
export async function getConfig(): Promise<AppConfig> {
  return ipcInvoke<AppConfig>('config:get')
}
