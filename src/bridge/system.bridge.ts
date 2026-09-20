import type { LocalDataUsage, PurgeReport, StorageInfo } from '@/types/system'
import { ipcInvoke } from '@/bridge'

/** 获取客户端存储信息（存储目录 / 日志路径 / 版本号），设置页「关于」使用 */
export async function getStorageInfo(): Promise<StorageInfo> {
  return ipcInvoke<StorageInfo>('get_storage_info')
}

/**
 * 重置客户端：清掉一切可重新从服务端获取的东西（三层缓存 + 公告基线 + 公告已读状态）。
 *
 * 不动工作区代码、提交源码快照、配置与登录会话。**不重拉** —— 补拉由调用方编排。
 */
export async function resetClient(): Promise<void> {
  return ipcInvoke<void>('reset_client')
}

/** 统计可清理的本地数据（清理前的预览） */
export async function localDataUsage(): Promise<LocalDataUsage> {
  return ipcInvoke<LocalDataUsage>('local_data_usage')
}

/**
 * 清理本地数据（**不可逆**）：日志内容 / 过期提交留档。
 *
 * 两个开关都由调用方显式传入，后端不做默认值兜底 —— 不可逆动作不接受隐式范围。
 */
export async function purgeLocalData(
  logs: boolean,
  staleSnapshots: boolean,
): Promise<PurgeReport> {
  return ipcInvoke<PurgeReport>('purge_local_data', { logs, staleSnapshots })
}
