import { defineStore } from 'pinia'
import { systemService } from '@/services/system.service'
import type {
  DataDirChange,
  DataDirInfo,
  LocalDataUsage,
  PurgeReport,
  StorageInfo,
} from '@/types/system'

/**
 * 系统服务的前端消费入口（2026-11-04，分层约定统一为 View → Store → Service）。
 *
 * 历史：窗口控制与启动标志曾由视图/组件直接调用 `systemService`，而 service
 * 的注释允许这种直连——与「View → Store → Service」的分层规则矛盾。现在
 * View / component / composable **一律经本 store**；service 只面向 store、
 * 守卫与组合根（不认识路由，也不承载视图调用）。
 *
 * 本 store 不复制状态：启动标志快照仍由 `systemService` 持有（守卫首航填充，
 * 早于任何组件挂载），`isKeyed` 只是转发读——避免两处快照漂移。
 */
export const useSystemStore = defineStore('system', {
  getters: {
    /**
     * 本次启动是否为 `--settings` 管理方钥匙（进程级判据）。
     *
     * 必须读**响应式快照**本身而不是调 `systemService.isKeyed()`：Pinia getter
     * 会缓存，方法调用不建立依赖——若在守卫填充快照前读过一次，会永久缓存
     * false。直接读 `launchFlags.value.settings` 才能随快照填充重新求值。
     */
    isKeyed: (): boolean => systemService.launchFlags.value.settings,
  },

  actions: {
    // ── 无边框窗口控制（View 经 store；失败无补救动作，调用方忽略） ──
    async minimizeWindow(): Promise<void> {
      await systemService.minimizeWindow()
    },
    async toggleMaximizeWindow(): Promise<void> {
      await systemService.toggleMaximizeWindow()
    },
    async closeWindow(): Promise<void> {
      await systemService.closeWindow()
    },
    async destroyWindow(): Promise<void> {
      await systemService.destroyWindow()
    },
    onCloseRequested(
      handler: (event: { preventDefault(): void }) => void,
    ): Promise<() => void> {
      return systemService.onCloseRequested(handler)
    },

    // ── 设置页：关于 / 维护 / 数据目录 ──
    getStorageInfo(): Promise<StorageInfo> {
      return systemService.getStorageInfo()
    },
    resetClient(): Promise<void> {
      return systemService.resetClient()
    },
    localDataUsage(): Promise<LocalDataUsage> {
      return systemService.localDataUsage()
    },
    purgeLocalData(purgeLogs: boolean, purgeSnapshots: boolean): Promise<PurgeReport> {
      return systemService.purgeLocalData(purgeLogs, purgeSnapshots)
    },
    getDataDir(): Promise<DataDirInfo> {
      return systemService.getDataDir()
    },
    pickDataDir(): Promise<string | null> {
      return systemService.pickDataDir()
    },
    setDataDir(target: string, migrate: boolean): Promise<DataDirChange> {
      return systemService.setDataDir(target, migrate)
    },
    resetDataDir(migrate: boolean): Promise<DataDirChange> {
      return systemService.resetDataDir(migrate)
    },
  },
})
