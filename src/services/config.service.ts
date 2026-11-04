import type { AppConfig } from '@/types/config'
import { computed, readonly, ref, shallowRef } from 'vue'
import type { Ref } from 'vue'
import { IpcError } from '@/bridge'
import * as configBridge from '@/bridge/config.bridge'
import { DEFAULT_LANGUAGE, normalizeHojLanguage } from '@/utils/language'
import {
  DEFAULT_EDITOR_FONT_SIZE,
  DEFAULT_EDITOR_TAB_SIZE,
  DEFAULT_EDITOR_THEME,
  EDITOR_FONT_SIZE_MAX,
  EDITOR_FONT_SIZE_MIN,
  EDITOR_TAB_SIZE_MAX,
  EDITOR_TAB_SIZE_MIN,
  normalizeEditorTheme,
} from '@/utils/editor'
import { createLogger } from '@/utils/logger'

/// 配置读取失败只影响局部展示/节奏，故一律 warn + 兜底值，不向上抛
const log = createLogger('configService')

/// 轮询参数兜底值，与 Rust `core::entity::config` 的默认值保持一致（2 秒 / 300 秒）
const DEFAULT_POLL_INTERVAL_MS = 2_000
const DEFAULT_POLL_TIMEOUT_MS = 300_000

/** 评测轮询调度参数 */
export interface PollSchedule {
  /** 相邻两次查询的间隔 */
  intervalMs: number
  /** 单次提交的轮询总时长上限，超时后停止（避免评测丢失时无限轮询） */
  timeoutMs: number
}

/** 切换结果：成功 / 已生效但持久化失败（前端据此决定是否提示重启回退） */
export type OjSwitchOutcome = 'success' | 'persist-failed'

/** 最近一次已生效切换的共享结果 */
export interface OjSwitchResult {
  target: string
  outcome: OjSwitchOutcome
}

/** 保存事务句柄：`runSaveTxn` 回调内**唯一**的保存通道（直通，不再二次排队） */
export interface SaveTxn {
  updateConfig(mutate: (draft: AppConfig) => void): Promise<AppConfig>
}

/**
 * 串行队列：任务按入队顺序逐个执行，后一个等前一个落地才开跑。
 *
 * 保存与切换两侧同构。原实现各自写一遍链式 Promise，并用两个手动同步的
 * 布尔量（`saveTail !== null` / `saveBusy`）表示同一件事——任一处漏改就会
 * 漂移。这里把「入队即置位 / 队尾落地才复位 / 等待捕获时刻的队尾」收敛成
 * 唯一实现。
 */
class SerialQueue {
  /// 队尾形态即唯一真相（`shallowRef` 避免 Vue 深度包装 Promise）
  private readonly tail = shallowRef<Promise<unknown> | null>(null)

  /** 排队或在途（入队即 true；队尾落地且无人再排队才 false）——**从队尾派生** */
  readonly inFlight: Readonly<Ref<boolean>> = readonly(computed(() => this.tail.value !== null))

  /**
   * 等待**调用时刻**的队尾落地（不向其后的新任务延伸）。
   *
   * 需要覆盖「调用之后才入队」的任务时，调用方必须自己复核代际
   *（`saveEpoch` / `ojSwitchEpoch`）——本方法不承诺这一点。
   */
  whenSettled(): Promise<void> {
    const tail = this.tail.value
    return tail ? tail.then(() => {}, () => {}) : Promise.resolve()
  }

  async enqueue<T>(task: () => Promise<T>): Promise<T> {
    const previous = this.tail.value
    let release!: () => void
    const gate = new Promise<void>((resolve) => {
      release = resolve
    })
    this.tail.value = gate
    try {
      if (previous) await previous.then(() => {}, () => {})
      return await task()
    } finally {
      release()
      // 只有自己仍是队尾才复位：排队在后面的任务已在入队时置位
      if (this.tail.value === gate) this.tail.value = null
    }
  }
}

/**
 * 是否为「OJ 切换已生效但配置持久化失败」错误。
 *
 * 只认 IPC 出口归一化的 `IpcError`：变体白名单在 `bridge`，裸读
 * `(e as { variant? }).variant` 会让非 IPC 异常（手写对象、未来包装器）冒充
 * 专用变体，把「切换未生效」误判成「已生效」。设置页与本服务共用本判据。
 */
export function isOjSwitchPersistFailed(error: unknown): boolean {
  return error instanceof IpcError && error.variant === 'OjSwitchPersistFailed'
}

/**
 * 配置服务 — 前端读取 Rust 端 `AppConfig` 的唯一入口，并提供派生参数。
 *
 * 分层约定：View / Store 一律不得直接调用 `config.bridge`，配置字段的读取与
 * 兜底逻辑集中在本服务，避免 IPC 细节与魔法数字散落各处。
 *
 * 缓存策略：配置属于应用级数据，由本服务在进程内缓存（缓存 Promise，使并发
 * 调用共享同一次 IPC）。设置页保存经 `runSaveTxn`（持保存门禁、覆盖
 * 「保存 → 自动切换」整段）落盘并失效缓存，其余读取方下次调用即拿到新值。
 *
 * 保存门禁：所有写入口（设置页保存、编辑器偏好、分栏比例）经 `runSaveTxn`
 * 走**同一队列**串行——并发的读-改-写会互相覆盖字段（丢更新）。与切换侧
 * （`switchOj` 排队 / `ojSwitchEpoch` / `whenOjSwitchSettled`）对称。
 *
 * **公开的排队入口有三个**：`runSaveTxn`（复合事务）与两个语义化包装
 * `updateEditorPrefs` / `saveSplitRatio`。三者都会等门禁，因此在
 * `runSaveTxn` 回调里调用**任意一个**都会排队等自己（自死锁）；回调内只能用
 * 参数 `tx.updateConfig`。这条规则由脚本门禁 `scripts/check-save-gate.mjs`
 *（`npm run check:save-gate`，CI frontend job 执行）锁定：回调正文出现任何
 * 排队入口即失败，并沿同文件局部辅助函数追踪传递路径（回调 → performSwitch →
 * 排队入口 会报出调用链）。跨文件辅助函数/别名不在文本扫描能力内。
 */
export class ConfigService {
  private cache: Promise<AppConfig> | null = null
  /// 切换队列：并发切换串行落地（跨控制器共享）
  private readonly switchQueue = new SerialQueue()
  /// 切换代际：每次切换**开始与结束**各 +1。读配置的调用方据此检测
  /// 「读取期间发生过切换」，从而在事务落地后重读，而不是把旧 active 展出。
  private switchEpoch = 0
  /// 最近一次**已生效**切换的共享结果：持久化失败是
  /// 「已生效但重启可能回退」的既存事实，设置页晚挂载也要据此提示；后续
  /// 成功切换或任意一次把同一 active 成功落盘的保存事务会解除它。
  private lastSwitchResult: OjSwitchResult | null = null
  /// 保存队列：所有写入口共用，后一次等前一次落地再执行
  private readonly saveQueue = new SerialQueue()
  /// 保存代际：每次保存事务**入队与结束**各 +1。读配置的调用方据此检测
  /// 「读取期间发生过保存」，事务落地后重读。排队中的事务也算开始——
  /// 它的意图已冻结，读取方不得忽略。
  private saveEpochCounter = 0
  /// 设置页发起、尚未落地的保存事务数（同一队列里也可能排在后台写入之后）
  private readonly settingsSaveDepth = ref(0)
  /// 设置页保存代际：**入队与结束**各 +1。后台写入不参与——设置页加载只关心
  /// 自己的保存（后台写入悬挂不得把它卡在 loading）
  private settingsSaveEpochCounter = 0
  /// 「等设置页保存落地」的等待者：设置页事务至多一个在途（save() 由 canSave 单飞）
  private readonly settingsSaveWaiters: Array<() => void> = []
  /** 保存队列状态（排队或在途均为 true；**含后台写入**，供观测/测试，不驱动设置页 UI） */
  readonly saveInFlight: Readonly<Ref<boolean>> = this.saveQueue.inFlight
  /**
   * **设置页发起**的保存事务是否在途（UI 的「保存中…」、保存按钮禁用、
   * 与 OJ 切换的双向互斥都用它）。与 `saveInFlight` 的区别：后者还包含
   * 编辑器偏好/分栏比例等**后台写入**——那些不该让设置页的下拉框与保存
   * 按钮产生反应（后台写入排在队列里等它落地即可，不构成用户意图冲突）。
   * 由 `settingsSaveDepth > 0` **派生**，没有独立布尔量可漂移。
   */
  readonly settingsSaveInFlight: Readonly<Ref<boolean>> = readonly(
    computed(() => this.settingsSaveDepth.value > 0),
  )

  /** 最近一次已生效切换的结果（未发生生效切换时为 null） */
  latestOjSwitchResult(): OjSwitchResult | null {
    return this.lastSwitchResult
  }

  /** 当前切换代际（只读；语义见 `switchEpoch`） */
  get ojSwitchEpoch(): number {
    return this.switchEpoch
  }

  /** 是否有 OJ 切换在途（跨控制器） */
  get ojSwitchInFlight(): boolean {
    return this.switchQueue.inFlight.value
  }

  /**
   * 等待在途 OJ 切换落地（不向等待者抛出切换错误）。
   *
   * 新挂载的设置页控制器在读取配置前必须等它：否则读到的可能是切换前的
   * 旧 active，而旧控制器的完成回调不会再刷新新控制器。只覆盖调用时刻已
   * 排队的切换，之后的切换由 `ojSwitchEpoch` 复查。
   */
  whenOjSwitchSettled(): Promise<void> {
    return this.switchQueue.whenSettled()
  }

  /** 当前保存代际（只读；语义见 `saveEpochCounter`） */
  get saveEpoch(): number {
    return this.saveEpochCounter
  }

  /**
   * 等待在途/排队中的保存事务落地（不向等待者抛出保存错误）。
   *
   * 只覆盖**调用时刻已排队**的事务：之后才入队的不在其中。需要连
   * 「等待期间新开始的保存」一并覆盖的调用方，必须自己复核 `saveEpoch`
   *（`useSettingsForm.load` 的循环即此）。
   */
  whenSaveSettled(): Promise<void> {
    return this.saveQueue.whenSettled()
  }

  /** 设置页保存代际（只读；语义见 `settingsSaveEpochCounter`） */
  get settingsSaveEpoch(): number {
    return this.settingsSaveEpochCounter
  }

  /**
   * 等待**设置页发起**的保存事务落地（不向等待者抛出保存错误）。
   *
   * 只等设置页自己的保存：后台写入（编辑器偏好/分栏比例）可能长时间不返回，
   * 不应把设置页卡在 loading（2026-11-04 评审：恢复旧 composable 的等待范围）。
   * 设置页事务至多一个在途（`save()` 由 `canSave` 单飞）。
   */
  whenSettingsSaveSettled(): Promise<void> {
    if (this.settingsSaveDepth.value === 0) return Promise.resolve()
    return new Promise((resolve) => this.settingsSaveWaiters.push(resolve))
  }

  /**
   * 保存事务：整个回调期间持有保存门禁，覆盖「保存 → 自动切换」这类
   * 复合操作（旧实现靠设置页 composable 的模块级 `saving` 手动圈住，
   * 编辑器偏好/分栏比例等第二保存入口绕过它——现在所有入口同一队列）。
   *
   * 回调内**只能**用 `tx.updateConfig` 保存（直通，不再二次排队）。
   * 注意排除面比「别调 `runSaveTxn`」更宽：`updateEditorPrefs` / `saveSplitRatio`
   * 这两个**语义化包装同样排队**，在回调内调用它们与嵌套 `runSaveTxn` 一样会
   * 排在自身门禁之后（自死锁）。禁止的写法由脚本门禁锁定
   *（`scripts/check-save-gate.mjs`：回调正文出现任何排队入口即失败，
   * 同文件局部辅助函数的传递调用同样追踪）。
   * `switchOj` 是独立队列，回调内调用安全（保存新建实例后的自动切换正是如此）。
   *
   * `options.settingsPage` 标记「本事务由设置页发起」：只有这类事务会置位
   * `settingsSaveInFlight` / `settingsSaveEpoch`，后台写入不参与设置页的 UI
   * 门禁；设置页加载也只等这类事务。
   */
  async runSaveTxn<T>(
    fn: (tx: SaveTxn) => Promise<T>,
    options: { settingsPage?: boolean } = {},
  ): Promise<T> {
    // 入队即视为开始：代际同步 +1（busy 由队列在 enqueue 内从队尾派生）。若等真正
    // 执行才置位，「已排队、busy 仍 false」的窗口里另一控制器的显式切换可
    // 插队落地，随后本事务仍按旧 target 自动切换——重演「较旧意图覆盖较新选择」。
    this.saveEpochCounter += 1
    if (options.settingsPage) {
      this.settingsSaveDepth.value += 1
      this.settingsSaveEpochCounter += 1
    }
    const tx: SaveTxn = { updateConfig: (mutate) => this.updateConfigNow(mutate) }
    try {
      return await this.saveQueue.enqueue(() => fn(tx))
    } finally {
      this.saveEpochCounter += 1
      if (options.settingsPage) {
        this.settingsSaveEpochCounter += 1
        this.settingsSaveDepth.value -= 1
        if (this.settingsSaveDepth.value === 0) {
          for (const resolve of this.settingsSaveWaiters.splice(0)) resolve()
        }
      }
    }
  }

  /** 读取应用配置（进程内缓存；失败时清空缓存以便下次重试） */
  getConfig(): Promise<AppConfig> {
    if (!this.cache) {
      this.cache = configBridge.getConfig().catch((e) => {
        this.cache = null
        throw e
      })
    }
    return this.cache
  }

  /** 使配置缓存失效（配置热重载后调用） */
  invalidate(): void {
    this.cache = null
  }

  /**
   * 切换当前 OJ（设置页「当前 OJ」下拉的显式动作）。
   *
   * 后端编排（整段在配置写事务内）：校验已注册且在启用实例列表 → 切 Registry
   * → 清各 Service 的 OJ 域缓存 → 持久化 `oj.active` → 发布 `OjSwitched`。
   * 本地缓存在切换后失效，后续读取拿到新 active。
   *
   * **并发调用排队**而不是并行发出：后一次切换等前一次落地再执行。后端已有
   * 写事务保证最终一致，这里保证 `whenOjSwitchSettled` 覆盖调用时刻已排队的
   * 全部切换（设置页新控制器等待时不会漏掉队尾）；其后才发起的切换由
   * `ojSwitchEpoch` 复查兜住。
   */
  async switchOj(ojId: string): Promise<void> {
    this.switchEpoch += 1
    try {
      await this.switchQueue.enqueue(async () => {
        try {
          await configBridge.switchOj(ojId)
        } catch (e) {
          // 切换已生效但持久化失败：后端 registry 与内存已是新 OJ，**陈旧缓存必须
          // 失效**——否则后续读取/保存会把旧 active 再写回去，造成 register=新、
          // 配置=旧 的再次分叉。结果同时留给设置页晚挂载提示。
          if (isOjSwitchPersistFailed(e)) {
            this.invalidate()
            this.lastSwitchResult = { target: ojId, outcome: 'persist-failed' }
          }
          // 普通失败（切换未生效）不覆盖既存的生效结果
          throw e
        }
        this.lastSwitchResult = { target: ojId, outcome: 'success' }
        this.invalidate()
      })
    } finally {
      this.switchEpoch += 1
    }
  }

  /**
   * OJ 基址：用于把题面/比赛简介中的相对图片 URL 改写为绝对地址。
   *
   * 取当前 OJ 实例（`oj.active` 匹配）的 `baseUrl`。读取失败时返回空串而非
   * 抛出 —— 缺基址只影响图片显示，不应阻断题面渲染。
   */
  async getOjBaseUrl(): Promise<string> {
    try {
      const config = await this.getConfig()
      return this.activeOjBaseUrl(config)
    } catch (e) {
      log.error('读取 OJ 基址失败，题面图片将保持相对路径:', e)
      return ''
    }
  }

  /**
   * 当前 OJ 实例的服务端地址（同步帮手，供已持有配置的调用方复用）。
   *
   * `active` 未命中实例列表时回退第一个启用实例 —— 正常配置经 Rust 归一
   * 不会出现未命中，此回退仅兜底 IPC 写读竞态。
   */
  activeOjBaseUrl(config: AppConfig): string {
    const enabled = config.oj.instances.filter((i) => i.enabled)
    const active = enabled.find((i) => i.id === config.oj.active)
    return (active ?? enabled[0])?.baseUrl ?? ''
  }

  /**
   * 评测轮询调度参数（间隔 + 总超时）。
   *
   * 读取失败或配置非法时回退兜底值，保证提交后轮询一定能启动。
   */
  async getPollSchedule(): Promise<PollSchedule> {
    try {
      const config = await this.getConfig()
      return {
        intervalMs: secsToMs(config.oj.pollIntervalSecs, DEFAULT_POLL_INTERVAL_MS),
        timeoutMs: secsToMs(config.oj.pollTimeoutSecs, DEFAULT_POLL_TIMEOUT_MS),
      }
    } catch (e) {
      log.error('读取轮询配置失败，使用兜底轮询参数:', e)
      return { intervalMs: DEFAULT_POLL_INTERVAL_MS, timeoutMs: DEFAULT_POLL_TIMEOUT_MS }
    }
  }

  /**
   * 编辑器偏好（字号 / Tab 宽度 / 编辑器主题）。
   *
   * 读取失败或值非法时回退兜底值（14 / 4 / 'vs'），编辑器渲染不因配置问题阻断。
   * 主题经 `normalizeEditorTheme` 收敛到 Monaco 主题值域 —— 未知主题名会让
   * `setTheme` 静默无效。
   */
  async getEditorPrefs(): Promise<EditorPrefs> {
    try {
      const config = await this.getConfig()
      return {
        fontSize: clampInt(
          config.editor?.fontSize,
          EDITOR_FONT_SIZE_MIN,
          EDITOR_FONT_SIZE_MAX,
          DEFAULT_EDITOR_FONT_SIZE,
        ),
        tabSize: clampInt(
          config.editor?.tabSize,
          EDITOR_TAB_SIZE_MIN,
          EDITOR_TAB_SIZE_MAX,
          DEFAULT_EDITOR_TAB_SIZE,
        ),
        editorTheme: normalizeEditorTheme(config.theme?.editorTheme),
      }
    } catch (e) {
      log.error('读取编辑器配置失败，使用兜底值:', e)
      return { fontSize: DEFAULT_EDITOR_FONT_SIZE, tabSize: DEFAULT_EDITOR_TAB_SIZE, editorTheme: DEFAULT_EDITOR_THEME }
    }
  }

  /**
   * 默认语言（HOJ 显示名，如 "C++"）。
   *
   * 值域归一：历史配置遗留的 Monaco id（'cpp' 等）经 `normalizeHojLanguage`
   * 映射回显示名，空值回退 "C++"；其它非空值（"Go"/"Rust"…）原样保留 ——
   * OJ 可能提供映射表之外的语言（值域约定见 `utils/language`）。
   */
  async getDefaultLanguage(): Promise<string> {
    try {
      const config = await this.getConfig()
      return normalizeHojLanguage(config.editor?.defaultLanguage)
    } catch (e) {
      log.error('读取默认语言失败，回退 C++:', e)
      return DEFAULT_LANGUAGE
    }
  }

  /**
   * 解题页初始分栏比例（左栏占比）。
   *
   * 非法值回退设计稿默认 0.48；读取失败同样回退，不阻断页面渲染。
   */
  async getSplitRatio(): Promise<number> {
    try {
      const config = await this.getConfig()
      const ratio = config.layout?.splitRatio
      return typeof ratio === 'number' && ratio >= 0.2 && ratio <= 0.8 ? ratio : DEFAULT_SPLIT_RATIO
    } catch (e) {
      log.error('读取分栏比例失败，使用默认值:', e)
      return DEFAULT_SPLIT_RATIO
    }
  }

  /**
   * 保存编辑器偏好（字号 / Tab 宽度 / 编辑器主题）—— 解题页编辑器设置弹层的落盘入口。
   *
   * 只写传入的字段，其余配置经事务句柄读-改-写保留。编辑器主题落在
   * `theme.editorTheme`（Monaco 主题）：客户端界面仍只有浅色，深色只作用于
   * 编辑器区域（旧版整机主题字段 themeName 已随 ThemeService 删除）。
   */
  async updateEditorPrefs(patch: Partial<EditorPrefs>): Promise<void> {
    await this.runSaveTxn((tx) => tx.updateConfig((draft) => {
      if (patch.fontSize !== undefined) draft.editor.fontSize = patch.fontSize
      if (patch.tabSize !== undefined) draft.editor.tabSize = patch.tabSize
      if (patch.editorTheme !== undefined) draft.theme.editorTheme = normalizeEditorTheme(patch.editorTheme)
    }))
  }

  /** 保存分栏比例（解题页拖拽结束后落盘；后台写入，不参与设置页门禁） */
  async saveSplitRatio(ratio: number): Promise<void> {
    await this.runSaveTxn((tx) => tx.updateConfig((draft) => {
      draft.layout.splitRatio = ratio
    }))
  }

  /**
   * 保存主体：读取当前配置 → 应用变更 → 整体写回后端 → 失效本地缓存。
   *
   * 后端 `update_config` 是整体替换语义，故必须先取当前值再改，
   * 避免局部字段把其余配置冲掉。返回写入后的配置供调用方直接使用。
   *
   * **只经 `runSaveTxn` 的回调句柄调用**（`tx.updateConfig`）：这是唯一不排队
   * 的写入通道。私有而非公开，是因为任何公开的保存方法都会排队——在事务回调
   * 里调它就等于等自己（自死锁）。
   */
  private async updateConfigNow(mutate: (draft: AppConfig) => void): Promise<AppConfig> {
    const current = await this.getConfig()
    const draft: AppConfig = structuredClone(current)
    mutate(draft)
    // 后端返回实际提交的配置（active 在写锁内钉住、可能为钳制后的值）；
    // 旧后端/测试桩返回空时退回 draft
    let persisted: AppConfig = draft
    try {
      persisted = (await configBridge.updateConfig(draft)) ?? draft
      // 用**实际落盘**的 active 判定：保存与切换在服务层
      // 可能交错，后端写锁内的 active 才是权威值；按旧快照判定会留下陈旧警告
      // 或返回过期 active。任意保存入口（含解题页偏好/分栏比例）都算解除。
      this.resolvePersistedSwitch(persisted.oj.active)
    } finally {
      // 无论成败都失效缓存：失败时下次读取重新拉后端真值，避免缓存与磁盘漂移
      this.invalidate()
    }
    return persisted
  }

  /// 成功保存同一 active 后解除持久化失败的共享结果。
  private resolvePersistedSwitch(active: string): void {
    const last = this.lastSwitchResult
    if (last?.outcome === 'persist-failed' && last.target === active) {
      this.lastSwitchResult = { target: active, outcome: 'success' }
    }
  }
}

/// 编辑器偏好（字号 / Tab 宽度 / 编辑器主题）
export interface EditorPrefs {
  fontSize: number
  tabSize: number
  /// Monaco 主题名（值域见 `utils/editor`）
  editorTheme: string
}

/// 分栏比例兜底值（设计稿 48% / 52%）
const DEFAULT_SPLIT_RATIO = 0.48

/// 整数钳位：非法值（非数字/越界）回退兜底
function clampInt(value: number | undefined, min: number, max: number, fallback: number): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) return fallback
  const rounded = Math.round(value)
  return rounded >= min && rounded <= max ? rounded : fallback
}

/// 秒 → 毫秒；非正数或非法值回退兜底
function secsToMs(secs: number | undefined, fallbackMs: number): number {
  return typeof secs === 'number' && secs > 0 ? secs * 1000 : fallbackMs
}

export const configService = new ConfigService()
