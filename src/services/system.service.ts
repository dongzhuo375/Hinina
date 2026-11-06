import { readonly, ref } from 'vue'
import type {
  DataDirChange,
  DataDirInfo,
  LaunchFlags,
  LocalDataUsage,
  PurgeReport,
  StorageInfo,
} from '@/types/system'
import * as systemBridge from '@/bridge/system.bridge'
import * as windowBridge from '@/bridge/window.bridge'

/**
 * 系统服务 — 客户端自身运行信息与维护操作
 * （存储目录 / 日志路径 / 版本号 / 重置客户端 / 清理本地数据 / 启动标志）。
 *
 * 分层约定（统一为严格 View → Store → Service）：View / component /
 * composable **一律经 `stores/systemStore` 消费**，不得直接调用本服务或 bridge；
 * 本服务只面向 store、守卫（`sessionGuard`）与
 * 组合根（`main.ts` 关窗握手）。不做缓存 —— 版本号构建期固定，但存储目录可能
 * 随用户数据迁移变化，设置页每次挂载都取实时值。**唯一例外是启动标志**：一次进程生命周期内不可变，
 * 且守卫与组件多处消费必须读到同一份，故 memoize 并维护响应式同步快照
 * （`launchFlags`，见下）。
 */

/// 本次启动标志的响应式快照：`getLaunchFlags` 首次解析即填充（守卫在首次导航
/// 时调用，**早于任何组件挂载**），此后同步可读。默认「无参数」= 安全侧
///（正常门禁 + 选手层）。
const launchFlagsSnapshot = ref<LaunchFlags>({ settings: false })

/// `getLaunchFlags` 的进程内单飞 Promise（含失败退化：保持快照默认值）
let launchFlagsOnce: Promise<LaunchFlags> | null = null

export class SystemService {
  /**
   * 启动标志同步快照（响应式）。守卫首航 `await getLaunchFlags()` 之后即为
   * 真值 —— 组件（ContestLayout 最小壳判据、SettingsView 分层锁）直接读这里，
   * 无需再等 Promise：异步判据会把闪帧反转到钥匙启动的首帧。
   */
  /// 只读视图：快照的「进程内不可变」由类型强制，外部拿不到写入口
  readonly launchFlags = readonly(launchFlagsSnapshot)

  /**
   * 本次启动是否为 `--settings` 管理方钥匙（**进程级**判据，无路由维度）。
   * 组件与守卫一律经 `systemStore` 读取（进程级 `isKeyed`；路由级组合
   * `isKeyedSettingsRoute` 也在 store——单一入口）。不要
   * 各处直接读 `launchFlags.value.settings`（判据要单一来源）。
   */
  isKeyed(): boolean {
    return launchFlagsSnapshot.value.settings
  }

  /** 获取客户端存储信息（设置页「关于」区块使用；失败由调用方降级展示） */
  async getStorageInfo(): Promise<StorageInfo> {
    return systemBridge.getStorageInfo()
  }

  /**
   * 读取本次启动的启动标志（守卫做初始导航/门禁豁免的唯一 Promise 入口；
   * 组件一律读同步快照 `launchFlags`）。
   *
   * **进程内 memoize**：标志不可变，多处消费必须同源。读取失败（旧二进制、
   * 桥异常）保持快照默认「无参数」—— 安全侧默认值，绝不让钥匙通道故障把
   * 启动或设置页卡死；守卫另有 try/catch 二道防线。
   */
  getLaunchFlags(): Promise<LaunchFlags> {
    launchFlagsOnce ??= systemBridge
      .getLaunchFlags()
      .then((flags) => {
        launchFlagsSnapshot.value = flags
        return flags
      })
      .catch(() => {
        // **不缓存 rejection**：标志在进程内不变，下次调用重试
        // 即可；一次瞬时失败不该让钥匙整个进程失效（否则维护者静默走普通门禁、
        // 字段全锁且无从得知原因）。快照在成功前保持安全侧默认值。
        launchFlagsOnce = null
        return launchFlagsSnapshot.value
      })
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

  /** 最小化窗口（无边框窗口的 UI 出口，失败无补救动作、由调用方忽略） */
  async minimizeWindow(): Promise<void> {
    return windowBridge.minimizeWindow()
  }

  /** 切换最大化 */
  async toggleMaximizeWindow(): Promise<void> {
    return windowBridge.toggleMaximizeWindow()
  }

  /** 关闭窗口（经 Tauri 关闭请求，由 close-guard 落盘后再退出） */
  async closeWindow(): Promise<void> {
    return windowBridge.closeWindow()
  }

  /** 销毁窗口（close-guard 落盘完成后的收尾动作） */
  async destroyWindow(): Promise<void> {
    return windowBridge.destroyWindow()
  }

  /** 订阅关闭请求（close-guard 的事件源；失败即「钩子没挂上」，由调用方记日志） */
  onCloseRequested(
    handler: (event: { preventDefault(): void }) => void,
  ): Promise<() => void> {
    return windowBridge.onCloseRequested(handler)
  }

  /** 弹出原生目录选择器（用户取消返回 null） */
  async pickDataDir(): Promise<string | null> {
    return systemBridge.pickDataDir()
  }

  /** 把 http(s) 外链转交系统默认浏览器（WebView 永不导航离开应用）。
   * scheme 白名单在 Rust 命令侧守卫；失败由调用方兜底（main.ts 仅记日志） */
  openExternal(url: string): Promise<void> {
    return systemBridge.openExternal(url)
  }
}

export const systemService = new SystemService()
