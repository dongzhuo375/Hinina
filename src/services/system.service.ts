import type { StorageInfo } from '@/types/system'
import * as systemBridge from '@/bridge/system.bridge'

/**
 * 系统服务 — 客户端自身运行信息与维护操作（存储目录 / 日志路径 / 版本号 / 清空缓存）。
 *
 * 分层约定：View / Store 不得直接调用 `system.bridge`，统一经本服务消费；
 * 不做缓存 —— 版本号构建期固定，但存储目录可能随用户数据迁移变化，设置页
 * 每次挂载都取实时值。
 */
export class SystemService {
  /** 获取客户端存储信息（设置页「关于」区块使用；失败由调用方降级展示） */
  async getStorageInfo(): Promise<StorageInfo> {
    return systemBridge.getStorageInfo()
  }

  /**
   * 清空客户端缓存（设置页「缓存」区块使用）。
   *
   * 只清可重新获取的数据；**不重拉** —— 清完由调用方决定何时补拉
   * （设置页清空后会立刻重拉当前比赛数据，否则界面会停在旧值上）。
   */
  async clearCache(): Promise<void> {
    return systemBridge.clearCache()
  }
}

export const systemService = new SystemService()
