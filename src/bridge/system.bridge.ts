import type { StorageInfo } from '@/types/system'
import { ipcInvoke } from '@/bridge'

/** 获取客户端存储信息（存储目录 / 日志路径 / 版本号），设置页「关于」使用 */
export async function getStorageInfo(): Promise<StorageInfo> {
  return ipcInvoke<StorageInfo>('get_storage_info')
}

/**
 * 清空客户端缓存（比赛列表/元信息、题面、题目 limits、终态提交详情与测试点）。
 *
 * 不动本地事实：工作区代码、提交源码快照、公告已读状态、配置、日志。
 */
export async function clearCache(): Promise<void> {
  return ipcInvoke<void>('clear_cache')
}
