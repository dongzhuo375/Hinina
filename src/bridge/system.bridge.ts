import type { StorageInfo } from '@/types/system'
import { ipcInvoke } from '@/bridge'

/** 获取客户端存储信息（存储目录 / 日志路径 / 版本号），设置页「关于」使用 */
export async function getStorageInfo(): Promise<StorageInfo> {
  return ipcInvoke<StorageInfo>('get_storage_info')
}
