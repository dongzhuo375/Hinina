import { defineStore } from 'pinia'
import {
  configService,
  isOjSwitchPersistFailed as classifyOjSwitchPersistFailed,
} from '@/services/config.service'
import type {
  EditorPrefs,
  OjSwitchResult,
  PollSchedule,
  SaveTxn,
} from '@/services/config.service'
import type { AppConfig } from '@/types/config'

/** 编辑器偏好（字号 / Tab 宽度 / 编辑器主题）——类型单一来源在 service，经 store 转出 */
export type { EditorPrefs } from '@/services/config.service'

/**
 * 配置服务的前端消费入口（分层约定统一为 View → Store → Service）。
 *
 * 历史：编辑器偏好、OJ 基址、分栏比例与设置页保存事务曾由视图/组件/composable
 * 直接调用 `configService`（Architecture.md 登记的分层欠账，2026-11-09 清偿）。
 * 现在 View / component / composable **一律经本 store**；service 面向 store 与
 * 其它 service（`contest.service`）。
 *
 * 本 store 不复制状态：保存队列、切换队列、代际计数与最近切换结果仍由
 * `configService` 持有（跨控制器共享的单一真值），这里只做消费入口的转发。
 *
 * **转发面按 service 状态的支撑形态分两类**（这是本 store 最容易写错的地方）：
 * - 由响应式 ref/computed 支撑的（`settingsSaveInFlight`、`ojSwitchInFlight`）
 *   走 **getter**：Pinia getter 是 computed，求值期间对 service ref 的读取会被
 *   依赖追踪，值翻转后自动重新求值（与 `systemStore.isKeyed` 同理）。
 * - 由**普通字段**支撑的（`ojSwitchEpoch`、`settingsSaveEpoch`、
 *   `latestOjSwitchResult`）必须走 **action（方法）**：computed 对无响应式依赖
 *   的求值结果会永久缓存——getter 化会把「代际比对/最近结果读取」钉死在首次
 *   读到的值上，`useSettingsForm` 的重读循环与持久化失败提示会静默失效。
 *   方法每次调用直读 service，无缓存。
 *
 * **action 一律非 async、直接 return service 调用**：调用同步进入 service、
 * 不加 await 壳，调用顺序与转发前一致（「invalidate → getConfig → 其它读取」
 * 的启动顺序与保存/切换的排队时序被既有用例锁定）。返回的 Promise 仍经 Pinia
 * action 包装、并非零微任务（实测多两跳——时序敏感断言须等终态而非数节拍）。
 */
export const useConfigStore = defineStore('config', {
  getters: {
    /**
     * 设置页发起的保存事务是否在途（覆盖「保存 → 自动切换」整段）。
     *
     * service 侧是 readonly computed ref；这里读 `.value` 让 computed 追踪到
     * 它。消费方需要 Ref 语义时用 `computed(() => store.settingsSaveInFlight)`
     * 包回——不要直接解构 store（拿到的是展开后的布尔快照）。
     */
    settingsSaveInFlight: (): boolean => configService.settingsSaveInFlight.value,

    /** 是否有 OJ 切换在途（跨控制器共享；service 侧由队列队尾派生 ref 支撑） */
    ojSwitchInFlight: (): boolean => configService.ojSwitchInFlight,
  },

  actions: {
    /** 当前切换代际（每次切换开始与结束各 +1）。普通字段，必须逐次直读，见类注释 */
    ojSwitchEpoch(): number {
      return configService.ojSwitchEpoch
    },

    /** 当前设置页保存代际（入队与结束各 +1；后台写入不参与）。普通字段，逐次直读 */
    settingsSaveEpoch(): number {
      return configService.settingsSaveEpoch
    },

    /** 最近一次已生效切换的共享结果（未发生生效切换时为 null）。service 方法读普通字段，非响应式——逐次直读 */
    latestOjSwitchResult(): OjSwitchResult | null {
      return configService.latestOjSwitchResult()
    },

    /** 「OJ 切换已生效但配置持久化失败」错误判据（判据单一来源在 service） */
    isOjSwitchPersistFailed(error: unknown): boolean {
      return classifyOjSwitchPersistFailed(error)
    },

    /** 读取应用配置（service 进程内缓存；并发调用共享同一次 IPC） */
    getConfig(): Promise<AppConfig> {
      return configService.getConfig()
    },

    /** 使配置缓存失效（配置热重载/OJ 切换后由 service 自理；视图无需调用，保留给设置页流程） */
    invalidate(): void {
      configService.invalidate()
    },

    /** 切换当前 OJ（并发调用在 service 排队串行落地） */
    switchOj(ojId: string): Promise<void> {
      return configService.switchOj(ojId)
    },

    /**
     * 保存事务：整个回调期间持有保存门禁，覆盖「保存 → 自动切换」复合操作。
     *
     * 回调内只能用 `tx.updateConfig` 保存——`runSaveTxn` / `updateEditorPrefs` /
     * `saveSplitRatio` 都排队，在回调内调用会等自己（自死锁），由
     * `scripts/check-save-gate.mjs` 静态锁定（本文件因此列入其 TXN_ALLOWLIST）。
     *
     * 实现注记：这里用 `(tx) => fn(tx)` 包一层而不是直接透传 `fn`——保存门禁
     * 的静态扫描无法解析「裸参数直传」的回调形态（fail closed 视为违规），
     * 内联箭头让回调保持可分析且语义不变。
     */
    runSaveTxn<T>(
      fn: (tx: SaveTxn) => Promise<T>,
      options: { settingsPage?: boolean } = {},
    ): Promise<T> {
      return configService.runSaveTxn((tx) => fn(tx), options)
    },

    /** 等待在途 OJ 切换落地（只覆盖调用时刻已排队的切换，其后切换由代际复查） */
    whenOjSwitchSettled(): Promise<void> {
      return configService.whenOjSwitchSettled()
    },

    /** 等待设置页发起的保存事务落地（后台写入不参与，不得把设置页卡在 loading） */
    whenSettingsSaveSettled(): Promise<void> {
      return configService.whenSettingsSaveSettled()
    },

    /** OJ 基址（题面/简介相对图片地址改写用；读取失败返回空串，不阻断渲染） */
    getOjBaseUrl(): Promise<string> {
      return configService.getOjBaseUrl()
    },

    /** 评测轮询调度参数（间隔 + 总超时；失败回退兜底值） */
    getPollSchedule(): Promise<PollSchedule> {
      return configService.getPollSchedule()
    },

    /** 编辑器偏好（字号 / Tab 宽度 / 主题；失败回退兜底值） */
    getEditorPrefs(): Promise<EditorPrefs> {
      return configService.getEditorPrefs()
    },

    /** 保存编辑器偏好（编辑器设置弹层的落盘入口；后台写入，不参与设置页门禁） */
    updateEditorPrefs(patch: Partial<EditorPrefs>): Promise<void> {
      return configService.updateEditorPrefs(patch)
    },

    /** 解题页初始分栏比例（非法/失败回退 0.48） */
    getSplitRatio(): Promise<number> {
      return configService.getSplitRatio()
    },

    /** 保存分栏比例（拖拽结束后落盘；后台写入，不参与设置页门禁） */
    saveSplitRatio(ratio: number): Promise<void> {
      return configService.saveSplitRatio(ratio)
    },
  },
})
