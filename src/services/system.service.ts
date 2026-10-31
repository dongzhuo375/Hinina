import type {
  DataDirChange,
  DataDirInfo,
  LaunchFlags,
  LocalDataUsage,
  PurgeReport,
  StorageInfo,
} from '@/types/system'
import * as systemBridge from '@/bridge/system.bridge'

/**
 * 系统服务 — 客户端自身运行信息与维护操作
 * （存储目录 / 日志路径 / 版本号 / 重置客户端 / 清理本地数据 / 启动标志）。
 *
 * 分层约定：View / Store 不得直接调用 `system.bridge`，统一经本服务消费；
 * 不做缓存 —— 版本号构建期固定，但存储目录可能随用户数据迁移变化，设置页
 * 每次挂载都取实时值。**唯一例外是 `getLaunchFlags`**：启动标志在一次进程
 * 生命周期内不可变，且守卫与设置页两处消费必须读到同一份，故在进程内
 * memoize（含失败退化，见方法注释）。
 */

/// `getLaunchFlags` 的进程内单飞 Promise（含失败退化结果）
let launchFlagsOnce: Promise<LaunchFlags> | null = null

export class SystemService {
  /** 获取客户端存储信息（设置页「关于」区块使用；失败由调用方降级展示） */
  async getStorageInfo(): Promise<StorageInfo> {
    return systemBridge.getStorageInfo()
  }

  /**
   * 读取本次启动的启动标志（守卫做初始导航/门禁豁免、设置页做分层锁）。
   *
   * **进程内 memoize**：标志不可变，两处消费必须同源。读取失败（旧二进制、
   * 桥异常）按「无参数」退化 —— 正常门禁 + 选手层，这是安全侧默认值，
   * 绝不让钥匙通道故障把启动或设置页卡死。
   */
  getLaunchFlags(): Promise<LaunchFlags> {
    launchFlagsOnce ??= systemBridge.getLaunchFlags().catch(() => ({ settings: false }))
    return launchFlagsOnce
  }

  /**
   * 重置客户端（设置页「重置客户端」使用）。
   *
   * 只清可重新获取的东西；**不重拉** —— 清完由调用方编排补拉
   * （设置页清空后会立刻重拉当前比赛数据，否则界面会停在旧值上）。
   */
  async resetClient(): Promise<void> {
    return systemBridge.resetClient()
  }

  /** 统计可清理的本地数据（「清理本地数据」区块的预览） */
  async localDataUsage(): Promise<LocalDataUsage> {
    return systemBridge.localDataUsage()
  }

  /**
   * 清理本地数据（**不可逆**）：日志与崩溃记录 / 过期提交留档。
   *
   * 范围由调用方显式传入（不在服务层设默认值）：不可逆动作不接受隐式范围。
   */
  async purgeLocalData(logs: boolean, staleSnapshots: boolean): Promise<PurgeReport> {
    return systemBridge.purgeLocalData(logs, staleSnapshots)
  }

  /** 读取当前数据目录信息（设置页「数据目录」区块） */
  async getDataDir(): Promise<DataDirInfo> {
    return systemBridge.getDataDir()
  }

  /**
   * 更改数据目录（**重启后生效**）。
   *
   * 只做「校验 + 记下改动」，真正的搬运由下次启动完成 —— 运行中搬运会让新旧目录
   * 产生写入分叉（见 `infra::data_dir` 模块头注释）。
   */
  async setDataDir(path: string, migrate: boolean): Promise<DataDirChange> {
    return systemBridge.setDataDir(path, migrate)
  }

  /** 恢复默认数据目录（**重启后生效**） */
  async resetDataDir(migrate: boolean): Promise<DataDirChange> {
    return systemBridge.resetDataDir(migrate)
  }

  /** 弹出原生目录选择器（用户取消返回 null） */
  async pickDataDir(): Promise<string | null> {
    return systemBridge.pickDataDir()
  }
}

export const systemService = new SystemService()
