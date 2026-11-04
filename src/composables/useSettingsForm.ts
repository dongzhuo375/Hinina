import { computed, getCurrentScope, onScopeDispose, reactive, ref } from 'vue'
import { isKeyedSettingsRoute } from '@/guards/keyed-settings-route'
import type { Ref } from 'vue'
import { useTransientValue } from '@/composables/useTransientValue'
import { useRouter } from 'vue-router'
import { configService, isOjSwitchPersistFailed } from '@/services/config.service'
import { useAuthStore } from '@/stores/authStore'
import { resetSessionForOjSwitch } from '@/stores/session'
import { normalizeHojLanguage } from '@/utils/language'
import { EDITOR_FONT_SIZE_MAX, EDITOR_FONT_SIZE_MIN, EDITOR_TAB_SIZES } from '@/utils/editor'
import { errorMessage } from '@/utils/error'
import { ojSelectOptions } from '@/utils/oj'
import type { AppConfig, OjInstance } from '@/types/config'

export interface SettingsForm {
  activeOj: string
  ojUrl: string
  contestRef: string
  contestPassword: string
  timeoutSecs: string
  pollIntervalSecs: string
  pollTimeoutSecs: string
  cacheTtlSecs: string
  cacheProblemStatement: boolean
  fontSize: string
  tabSize: number
  defaultLanguage: string
  autoSave: boolean
  autoSaveIntervalSecs: string
  splitRatio: number
}

export type SettingsFormPatch = Partial<SettingsForm>

/// 分栏比例钳位到滑杆值域 [0.30, 0.70]，两位小数（与 step 0.01 对齐）
export function clampRatio(value: number): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) return 0.48
  return Math.min(0.7, Math.max(0.3, Math.round(value * 100) / 100))
}

/// 严格非负整数解析：拒绝空串/小数/负号/科学计数法等 Number() 会宽容接受的形式
export function parseIntStrict(raw: string): number | null {
  const trimmed = raw.trim()
  if (!/^\d+$/.test(trimmed)) return null
  const n = Number(trimmed)
  return Number.isSafeInteger(n) ? n : null
}

function intError(raw: string, min: number, max: number, label: string): string | null {
  const n = parseIntStrict(raw)
  if (n === null) return `${label}须为非负整数`
  if (n < min || n > max) return `${label}须在 ${min}–${max} 之间`
  return null
}

/// **OJ 域字段清单（单一来源）**：切换 OJ 时需要「以磁盘为准」
/// 的全部字段。`ojDomainDirty`、`resetOjFieldsFromDisk`、重载失败的对齐分支都
/// 从这里派生，新增字段只改一处。
const OJ_DOMAIN_KEYS = [
  'activeOj', 'ojUrl', 'contestRef', 'contestPassword',
  'timeoutSecs', 'pollIntervalSecs', 'pollTimeoutSecs', 'cacheTtlSecs',
  'cacheProblemStatement',
] as const satisfies readonly (keyof SettingsForm)[]

/// 把 `from` 的 OJ 域字段复制到 `to`（全局字段不动）
function copyOjDomain(from: SettingsForm, to: SettingsForm): void {
  // 键集合由 OJ_DOMAIN_KEYS 唯一维护；写入用宽松记录类型绕过「键并集不可写」
  const source = from as unknown as Record<string, string | boolean>
  const target = to as unknown as Record<string, string | boolean>
  for (const key of OJ_DOMAIN_KEYS) target[key] = source[key]
}

/// **跨控制器的 OJ 切换状态**（模块级单例）。设置页可卸载重挂：旧控制器的
/// 慢切换完成时，新控制器已经读到了旧 active；旧控制器只重置自己的表单，
/// 新控制器不会收到刷新。`switching` 必须是共享的，否则两个控制器各自的
/// 单飞互相看不见。
const sharedSwitching = ref(false)

/// **跨控制器的保存状态**：`save → 新建实例自动切换` 是**一个**
/// 用户操作，必须整段互斥。只共享 `switching` 时，旧控制器在途的保存仍可在新
/// 控制器完成显式切换之后执行自动切换，用较旧的意图覆盖较新的选择。
const sharedSaving = ref(false)

/// 保存代际：开始/结束各 +1。新控制器读取配置时用它检测「读取期间有保存开始
/// 或结束」，避免在同步检查与异步读取之间漏掉一次保存。
let saveEpoch = 0

/// 等待在途保存落地（新控制器挂载时用：先等旧控制器把配置写完，再读取）
const saveWaiters: Array<() => void> = []
function whenSaveSettled(): Promise<void> {
  if (!sharedSaving.value) return Promise.resolve()
  return new Promise((resolve) => saveWaiters.push(resolve))
}

/// 切换结果：成功 / **已生效但持久化失败**。后者必须跨控制器
/// 传递——旧控制器卸载后无法设置警告，新控制器重读后仍要提示重启可能回退。
type SwitchOutcome = 'success' | 'persist-failed'

/// 切换完成通知：让**仍挂载的其它控制器**重读 OJ 域（发起者自己已处理）。
/// 返回 Promise：发起者会 await 全部观察者，期间共享 busy 不解除。
type SwitchObserver = (
  target: string,
  initiator: number,
  outcome: SwitchOutcome,
  epoch: number,
) => Promise<void> | void
const switchObservers = new Set<SwitchObserver>()
let controllerSeq = 0

const PERSIST_FAILED_NOTICE =
  '切换已生效，但配置保存失败（重启后可能回退）；连接与比赛相关的修改已重置，其余未保存修改保留'
const FOREIGN_RELOAD_FAILED_NOTICE =
  'OJ 已在另一处完成切换；配置刷新失败，请重新打开设置页确认'

export const SETTINGS_INPUT =
  'w-full rounded-sm border border-[var(--border-color)] bg-[var(--bg-card)] px-3 py-2 text-sm text-[var(--text-primary)] transition-colors placeholder-[var(--text-muted)] focus:border-[var(--color-primary)] focus:outline-none focus:ring-1 focus:ring-[var(--color-primary)] disabled:cursor-not-allowed disabled:bg-[var(--bg-sidebar)] disabled:text-[var(--text-muted)]'
const INPUT = SETTINGS_INPUT
const INPUT_ERROR = 'border-[var(--color-wa-border)] focus:border-[var(--color-wa-border)] focus:ring-[var(--color-error)]'

/**
 * 设置页中央配置草稿与唯一保存/OJ 切换事务。
 *
 * 四个配置 section 只编辑这一份 reactive 草稿；baseline/dirty/save/discard 始终读取
 * 同一对象。保存新建实例后的自动切换也留在这里，避免跨控制器回读事务状态。
 */
export function useSettingsForm(maintainerLocked: Readonly<Ref<boolean>> = ref(false)) {
  // 维护者层是否锁定（普通启动 = true）。锁定字段禁用且不可编辑，它们的**非法
  // 存量值不得阻塞可编辑字段的保存**（手改越界 config.json 时字体/
  // 布局也存不了、只能 --settings 重启——与 autoSaveIntervalSecs 的既有规则
  // 「禁用字段不参与校验」矛盾）。
  const locked = maintainerLocked
  const form = reactive<SettingsForm>({
    activeOj: 'HOJ',
    ojUrl: '',
    contestRef: '',
    contestPassword: '',
    timeoutSecs: '30',
    pollIntervalSecs: '2',
    pollTimeoutSecs: '300',
    cacheTtlSecs: '60',
    cacheProblemStatement: true,
    fontSize: '14',
    tabSize: 4,
    defaultLanguage: 'C++',
    autoSave: true,
    autoSaveIntervalSecs: '30',
    splitRatio: 0.48,
  })

  const baseline = ref('')
  function snapshot(): string {
    return JSON.stringify(form)
  }
  const dirty = computed(() => baseline.value !== '' && snapshot() !== baseline.value)

  /// 保存进行中（跨控制器共享）：覆盖 `save → 自动切换` 整段，
  /// 新挂载的控制器在旧控制器保存完成前不得发起切换/保存（否则旧意图可覆盖
  /// 用户较新的选择）。
  const saving = sharedSaving
  const saveError = ref<string | null>(null)
  const { value: showSaved, show: markSaved, clear: clearSaved } = useTransientValue(false, 3_000)

  const errors = computed<Record<string, string | null>>(() => ({
    // 锁定字段（不可编辑）不参与校验：与 autoSaveIntervalSecs 同一规则
    ojUrl: locked.value
      ? null
      : /^https?:\/\/\S+$/i.test(form.ojUrl.trim())
        ? null
        : '须以 http:// 或 https:// 开头',
    contestRef: null,
    timeoutSecs: locked.value ? null : intError(form.timeoutSecs, 1, 120, '请求超时'),
    pollIntervalSecs: locked.value ? null : intError(form.pollIntervalSecs, 1, 30, '轮询间隔'),
    pollTimeoutSecs: locked.value ? null : intError(form.pollTimeoutSecs, 30, 3600, '轮询总超时'),
    cacheTtlSecs: locked.value ? null : intError(form.cacheTtlSecs, 0, 600, '缓存 TTL'),
    fontSize: intError(form.fontSize, EDITOR_FONT_SIZE_MIN, EDITOR_FONT_SIZE_MAX, '字号'),
    // 关闭自动保存时该字段被禁用，隐藏值不得阻止无关字段保存
    autoSaveIntervalSecs: form.autoSave
      ? intError(form.autoSaveIntervalSecs, 5, 300, '自动保存间隔')
      : null,
  }))
  const isValid = computed(() => Object.values(errors.value).every((e) => e === null))
  /// OJ 切换进行中（跨控制器共享）：切换期间禁止
  /// 保存/再次切换——否则保存会用旧缓存构造带旧 active 的整份配置，与在途
  /// switch_oj 交错写盘。共享使**新挂载的控制器**也看见旧控制器在途的切换。
  const switching = sharedSwitching
  const canSave = computed(
    () => dirty.value && isValid.value && !saving.value && !switching.value,
  )

  /// 控制器所有权：设置视图卸载后，旧控制器的异步完成回调
  /// 不得再回写表单（视图已销毁，回写只会让状态机更难推理）；新控制器经共享
  /// 在途 Promise 等待事务落地后重读。
  const controllerId = ++controllerSeq
  let disposed = false
  const observer: SwitchObserver = (target, initiator, outcome, epoch) => {
    if (initiator === controllerId) return
    return adoptForeignSwitch(target, outcome, epoch)
  }
  if (getCurrentScope()) {
    onScopeDispose(() => {
      disposed = true
      switchObservers.delete(observer)
    })
    switchObservers.add(observer)
  }

  const loading = ref(true)
  const loadError = ref<string | null>(null)
  const router = useRouter()

  const ojInstances = ref<OjInstance[]>([])
  const ojOptions = computed(() => ojSelectOptions(ojInstances.value))
  const persistedActive = ref('HOJ')
  const switchError = ref<string | null>(null)
  const switchNotice = ref<string | null>(null)

  /// 最近一次自动/显式切换后是否停留在设置页（决定是否给出「未保存修改已重载」提示）
  let lastSwitchStayed = false

  /// OJ 域字段是否相对 baseline 有未保存修改（全局字段不算——它们被保留）
  function ojDomainDirty(): boolean {
    if (baseline.value === '') return false
    const base = JSON.parse(baseline.value) as SettingsForm
    // activeOj 由下拉 v-model 主动改变，不算「被覆盖的草稿」
    return OJ_DOMAIN_KEYS.filter((key) => key !== 'activeOj').some(
      (key) => JSON.stringify(base[key]) !== JSON.stringify(form[key]),
    )
  }

  /// 切换**已生效**但后续步骤失败时的统一对齐：
  /// OJ 域字段回到旧基线的磁盘值（不把旧 OJ 的草稿写进新 OJ），active/url 对齐
  /// 到新 OJ，基线按「磁盘值 + 新 active/url」重算（不产生假 dirty），全局草稿保留。
  function adoptEffectiveSwitch(target: string, notice: string): void {
    // 已卸载的旧控制器不再回写；仍挂载的控制器由
    // adoptForeignSwitch 走同一对齐逻辑
    if (disposed) return
    const previous = baseline.value ? (JSON.parse(baseline.value) as SettingsForm) : null
    persistedActive.value = target
    // 无基线（理论不可达）：只把 active/url 对齐到新 OJ 的已知值
    if (previous) copyOjDomain(previous, form)
    form.activeOj = target
    form.ojUrl = ojInstances.value.find((i) => i.id === target)?.baseUrl ?? ''
    if (previous) {
      baseline.value = JSON.stringify({ ...previous, activeOj: target, ojUrl: form.ojUrl })
    }
    switchError.value = null
    switchNotice.value = notice
  }

  /// 另一个控制器完成了切换（本实例仍挂载）：OJ 域以磁盘为准重读。
  /// 刷新失败时至少把 active/url 对齐到已知目标，不让界面停在旧 OJ。
  ///
  /// **代际检查**：`epoch` 是发起切换落地时的切换代际；若重读
  /// 期间又发生了切换，旧结果（尤其是旧目标的失败对齐）不得覆盖新状态——新
  /// 切换自己的通知会负责对齐与提示。
  async function adoptForeignSwitch(
    target: string,
    outcome: SwitchOutcome,
    epoch: number,
  ): Promise<void> {
    if (disposed) return
    try {
      await resetOjFieldsFromDisk()
      if (
        outcome === 'persist-failed' &&
        !disposed &&
        configService.ojSwitchEpoch === epoch
      ) {
        // 重读成功会清空提示；持久化失败的事实必须保留
        switchNotice.value = PERSIST_FAILED_NOTICE
      }
    } catch {
      if (disposed || configService.ojSwitchEpoch !== epoch) return
      adoptEffectiveSwitch(
        target,
        outcome === 'persist-failed' ? PERSIST_FAILED_NOTICE : FOREIGN_RELOAD_FAILED_NOTICE,
      )
    }
  }

  /// 读取**切换落地后**的配置：等待在途切换，并用代际检测
  /// 「读取期间新开始的切换」——否则新控制器可能把切换前的旧 active 读进表单，
  /// 而旧控制器的完成回调不会再刷新它。失败由调用方处理。
  async function readSettledConfig(): Promise<AppConfig> {
    for (;;) {
      const epoch = configService.ojSwitchEpoch
      // 无在途切换时**同步**走到 invalidate/getConfig：设置页其余读取的启动
      // 顺序被既有用例锁定（invalidate → config → 其它），不能平白多一个微任务
      if (configService.ojSwitchInFlight) {
        await configService.whenOjSwitchSettled()
        if (epoch !== configService.ojSwitchEpoch) continue
      }
      configService.invalidate()
      const config = await configService.getConfig()
      if (epoch === configService.ojSwitchEpoch) return config
    }
  }

  async function performSwitch(target: string): Promise<boolean> {
    // 单飞放在这里而非 onSwitchOj：保存新建实例后的**自动切换**也走本函数——
    // 只包显式切换会让保存后的自动切换绕开门禁。
    // 共享的 `switching` 覆盖跨控制器（旧设置页的切换在途时新页面也不得再发）。
    if (switching.value) {
      await configService.whenOjSwitchSettled()
      return false
    }
    switching.value = true
    lastSwitchStayed = false
    // 切换结果（成功 / 已生效但持久化失败）：决定是否/如何通知其它控制器
    let outcome: SwitchOutcome | null = null
    try {
      await configService.switchOj(target)
      outcome = 'success'
      // 会话上下文先清理（领域状态 + 认证态，两者分离见各自文档）；
      // persistedActive 与 OJ 域在重置/对齐成功后才提交
      //（表单与 persistedActive 必须一起变化，否则「B 的 id +
      // A 的地址」会被下一次保存写坏。注意重载失败**不回滚到旧 OJ**——
      // 切换已生效，由 adoptEffectiveSwitch 对齐新 OJ）
      resetSessionForOjSwitch()
      useAuthStore().resetForOjSwitch()
      // `--settings`（管理方钥匙）：切换成功**留在设置页**继续修配置——踢到
      // 登录页会打断维护、丢失当前草稿（登录页的「返回设置」是一次重新进入）。会话已由
      // resetSessionForOjSwitch + resetForOjSwitch 清理，最小壳接管（内容区共享实例，草稿保留）。
      // 第三处豁免导航源，与 sessionGuard/阶段闸门同理由。
      // **豁免只属于设置页本身**：switch_oj 是异步的，期间用户可能已导航到
      // 比赛页——不查当前路由就把「停留」带过去，无会话用户会被留在比赛页
      //。判据与 sessionGuard 一致：钥匙 + 当前路由是设置页。
      const stayOnSettings = isKeyedSettingsRoute(router.currentRoute.value?.name)
      if (!stayOnSettings) {
        persistedActive.value = target
        void router.replace({ name: 'Login' })
        return true
      }
      // 留在设置页时**只重置 OJ 域并按磁盘重算 baseline**（防假
      // dirty；保留全局草稿）：整体 load() 会把维护者刚改的字体/
      // 超时也一并还原，与回滚分支的「保留编辑」行为不一致。
      try {
        await resetOjFieldsFromDisk()
      } catch {
        adoptEffectiveSwitch(
          target,
          '切换已生效，但配置刷新失败；连接与比赛相关的修改已重置，其余未保存修改保留。请重新打开设置页确认',
        )
        lastSwitchStayed = true
        return true
      }
      lastSwitchStayed = true
      return true
    } catch (e) {
      // 后端已切 registry、清缓存，只是持久化 oj.active 失败：切换**已生效**，
      // 必须走「承认已生效」分支（并补做 resetSessionForOjSwitch——否则旧 OJ
      // 的登录态会被带到新 OJ）
      if (isOjSwitchPersistFailed(e)) {
        outcome = 'persist-failed'
        resetSessionForOjSwitch()
        useAuthStore().resetForOjSwitch()
        if (isKeyedSettingsRoute(router.currentRoute.value?.name)) {
          adoptEffectiveSwitch(target, PERSIST_FAILED_NOTICE)
          lastSwitchStayed = true
          return true
        }
        // 非设置页（正常流程）：照旧回登录页；会话已重置
        void router.replace({ name: 'Login' })
        return true
      }
      switchError.value = errorMessage(e, '切换 OJ 失败')
      return false
    } finally {
      try {
        // 仍挂载的其它控制器（例如切换期间重新进入设置页的新实例）重读 OJ 域；
        // **等待它们完成**再解除共享 busy（等待观察者完成再解除 busy）——否则重读期间输入
        // 已重新启用，`copyOjDomain` 会再次覆盖用户刚输入的内容。发起者自己
        // 已在上面完成重置/对齐。
        if (outcome) {
          const epoch = configService.ojSwitchEpoch
          // 共享结果已由 ConfigService.switchOj 记录；这里只通知仍挂载的控制器
          const listeners = [...switchObservers]
          await Promise.allSettled(
            listeners.map((listen) => listen(target, controllerId, outcome!, epoch)),
          )
        }
      } finally {
        switching.value = false
      }
    }
  }

  async function onSwitchOj(): Promise<void> {
    // 单飞：切换进行中、或正在保存时，再次选择直接忽略并回退下拉
    //（两条写路径必须双向互斥）
    if (switching.value || saving.value) {
      form.activeOj = persistedActive.value
      return
    }
    switchError.value = null
    switchNotice.value = null
    const target = form.activeOj
    const option = ojOptions.value.find((o) => o.id === target)
    if (option && !option.configured) {
      const discarded = form.ojUrl !== ''
      form.ojUrl = ''
      switchNotice.value =
        `${target} 尚未配置：填写服务器地址后点「保存」，将自动创建实例并切换` +
        (discarded ? '（切换前未保存的地址编辑已丢弃）' : '')
      return
    }
    const ojDirty = ojDomainDirty()
    const switched = await performSwitch(target)
    if (switched) {
      form.ojUrl = ojInstances.value.find((i) => i.id === target)?.baseUrl ?? ''
      if (lastSwitchStayed && ojDirty && switchNotice.value === null) {
        // OJ 域草稿被磁盘值覆盖是设计（换服务端后旧值无意义），
        // 但不能静默——给一次说明，并点明全局草稿仍保留
        switchNotice.value =
          '切换已完成；连接与比赛相关的未保存修改已从磁盘重新加载（编辑器/布局等修改保留）'
      }
    } else {
      // 切换失败（未生效）：只回滚**选择**，不动地址草稿——
      // 用磁盘地址覆盖会静默丢掉用户刚编辑的 A 地址；持久化失败/重载失败
      // 的「已生效」分支在 performSwitch 内部已完成对齐
      form.activeOj = persistedActive.value
    }
  }

  /// config → 完整表单值的映射（populate 与「切换后重置 OJ 域」共用）
  function diskForm(config: AppConfig): SettingsForm {
    const draft: SettingsForm = { ...form }
    draft.activeOj = config.oj.active
    draft.ojUrl = config.oj.instances.find((i) => i.id === config.oj.active)?.baseUrl ?? ''
    draft.contestRef = config.oj.contestRef ?? ''
    draft.contestPassword = config.oj.contestPassword ?? ''
    draft.timeoutSecs = String(config.oj.timeoutSecs)
    draft.pollIntervalSecs = String(config.oj.pollIntervalSecs)
    draft.pollTimeoutSecs = String(config.oj.pollTimeoutSecs)
    draft.cacheTtlSecs = String(config.oj.cacheTtlSecs)
    draft.cacheProblemStatement = config.oj.cacheProblemStatement ?? true
    draft.fontSize = String(config.editor.fontSize)
    draft.tabSize = EDITOR_TAB_SIZES.includes(config.editor.tabSize) ? config.editor.tabSize : 4
    draft.defaultLanguage = normalizeHojLanguage(config.editor.defaultLanguage)
    draft.autoSave = config.editor.autoSave
    draft.autoSaveIntervalSecs = String(config.editor.autoSaveIntervalSecs)
    draft.splitRatio = clampRatio(config.layout.splitRatio)
    return draft
  }

  /// 切换 OJ 后：OJ 域字段以磁盘新值为准，**编辑器/布局等全局草稿保留**
  /// （整体 load() 会丢掉维护者刚改的字体/超时；回滚分支本就保留，
  /// 两条路径对齐）。baseline 按「磁盘值 + 保留的全局草稿」计算——dirty 只反映
  /// 真正未保存的全局编辑，且不会把旧 OJ 的 OJ 域草稿写到新 OJ。
  /// 读取前等待在途切换落地（`readSettledConfig`）；卸载后不再回写。
  async function resetOjFieldsFromDisk(): Promise<void> {
    const config = await readSettledConfig()
    if (disposed) return
    const disk = diskForm(config)
    copyOjDomain(disk, form)
    persistedActive.value = disk.activeOj
    switchError.value = null
    switchNotice.value = null
    ojInstances.value = config.oj.instances
    // baseline = 磁盘全量值；form 的全局字段仍是草稿 → dirty 即「未保存的全局编辑」
    baseline.value = JSON.stringify(disk)
  }

  function populate(config: AppConfig): void {
    Object.assign(form, diskForm(config))
    persistedActive.value = config.oj.active
    switchError.value = null
    // 持久化失败警告描述的是「当前已生效但重启可能回退」的事实；加载/重读不得
    // 把它吞掉（结果跨控制器传递）。成功切换会经 resetOjFieldsFromDisk 清空。
    if (switchNotice.value !== PERSIST_FAILED_NOTICE) switchNotice.value = null
    ojInstances.value = config.oj.instances
    baseline.value = snapshot()
  }

  /// 晚挂载控制器恢复历史持久化失败警告：共享结果由
  /// `ConfigService` 持有（任意 updateConfig 成功也能解除），
  /// 这里只在仍是同一目标的 persist-failed 时提示。
  function restoreHistoricalSwitchNotice(config: AppConfig): void {
    const last = configService.latestOjSwitchResult()
    if (!last || last.outcome !== 'persist-failed') return
    if (config.oj.active !== last.target) return
    switchNotice.value = PERSIST_FAILED_NOTICE
  }

  /// 普通保存成功后清除本控制器可见的“重启可能回退”警告：解除判定已由
  /// `ConfigService.updateConfig` 统一完成（覆盖设置页之外的保存入口）。
  function clearResolvedPersistedWarning(persistedActiveId: string): void {
    if (switchNotice.value !== PERSIST_FAILED_NOTICE) return
    const last = configService.latestOjSwitchResult()
    if (last?.outcome === 'success' && last.target === persistedActiveId) {
      switchNotice.value = null
    }
  }

  async function load(): Promise<void> {
    loading.value = true
    loadError.value = null
    try {
      // 新挂载的控制器若撞上旧控制器的在途保存（含自动切换），先等它落地：
      // 否则读到的是保存前的配置，而普通保存不会发切换通知来刷新本页。
      // 无保存时**同步**通过，保持既有「invalidate → getConfig → 其它读取」
      // 的启动顺序（SettingsView 行为用例锁定）；读完后若又有保存开始，重读。
      let config: AppConfig
      for (;;) {
        if (saving.value) await whenSaveSettled()
        // 等待结束后再捕获代际：保存结束必然 +1，若沿用等待前的值会固定多读一次
        const epoch = saveEpoch
        config = await readSettledConfig()
        // 读取期间若有保存开始/结束（epoch 变化）或仍有保存在途，重读
        if (!saving.value && epoch === saveEpoch) break
      }
      if (disposed) return
      populate(config)
      restoreHistoricalSwitchNotice(config)
    } catch (e) {
      if (!disposed) loadError.value = errorMessage(e, '读取配置失败')
    } finally {
      loading.value = false
    }
  }

  async function save(): Promise<void> {
    if (!canSave.value || switching.value) return
    saving.value = true
    saveEpoch += 1
    saveError.value = null
    // 固定提交快照：保存期间的后续编辑不进入本次落盘，也不得被误标为已保存
    const submitted = { ...form }
    const submittedSnapshot = snapshot()
    const target = submitted.activeOj
    // 锁定字段：不可编辑，写回未改动值即可——但**不参与数字解析**（存量非法
    // 字符串会变 NaN → JSON null → Rust u64 反序列化失败拖垮整次保存；
    //）。保持磁盘原值不动。
    const lockAll = locked.value
    // 锁定态不会创建实例（载荷里 `if (!lockAll)` 跳过写入），因此也**不得**
    // 走自动切换：否则 switch_oj 对不存在的实例失败，会谎报「实例已保存；
    // 切换未成功」（手改 active 指向缺失实例时可达）
    const isNewInstance = !lockAll && !ojInstances.value.some((i) => i.id === target)
    try {
      const saved = await configService.updateConfig((draft) => {
        // active 只由 switch_oj 改写；保存不能制造配置与运行中 Registry 的不一致
        if (!lockAll) {
          const instance = draft.oj.instances.find((i) => i.id === target)
          if (instance) {
            instance.baseUrl = submitted.ojUrl.trim()
          } else {
            draft.oj.instances.push({
              id: target,
              baseUrl: submitted.ojUrl.trim(),
              enabled: true,
              options: {},
            })
          }
        }
        draft.oj.contestRef = submitted.contestRef.trim()
        draft.oj.contestPassword = submitted.contestPassword === '' ? null : submitted.contestPassword
        if (!lockAll) {
          draft.oj.timeoutSecs = Number(submitted.timeoutSecs)
          draft.oj.pollIntervalSecs = Number(submitted.pollIntervalSecs)
          draft.oj.pollTimeoutSecs = Number(submitted.pollTimeoutSecs)
          draft.oj.cacheTtlSecs = Number(submitted.cacheTtlSecs)
          draft.oj.cacheProblemStatement = submitted.cacheProblemStatement
        }
        draft.editor.fontSize = Number(submitted.fontSize)
        draft.editor.tabSize = submitted.tabSize
        draft.editor.defaultLanguage = submitted.defaultLanguage
        draft.editor.autoSave = submitted.autoSave
        // 关闭自动保存时该字段被禁用且不参与校验：隐藏的原始输入不得进入载荷
        // （NaN 会序列化成 null，Rust u64 反序列化失败会让整次保存失败），保留已存值
        const autoSaveIntervalSecs = submitted.autoSave
          ? parseIntStrict(submitted.autoSaveIntervalSecs)
          : null
        if (autoSaveIntervalSecs !== null) {
          draft.editor.autoSaveIntervalSecs = autoSaveIntervalSecs
        }
        draft.layout.splitRatio = submitted.splitRatio
      })
      // 保存已把当前 active 落盘：若此前是「切换生效但持久化失败」，回退风险
      // 已由 ConfigService.updateConfig 解除；这里只清本控制器可见的警告
      //（即便控制器已卸载也不会影响服务级共享结果）。
      clearResolvedPersistedWarning(saved.oj.active)
      // 已卸载的旧控制器不再回写表单（跨控制器共享）；自动切换仍执行——
      // 那是保存新建实例的既定后果，且切换本身与视图生命周期无关
      if (!disposed) {
        baseline.value = submittedSnapshot
        ojInstances.value = saved.oj.instances
        markSaved(true)
      }

      if (isNewInstance) {
        if (await performSwitch(target)) return
        // 后端仍停留在旧 OJ：下拉和地址必须一起回滚，否则下一次无关保存会把
        // 新实例地址写进旧实例。baseline 同步到“已保存配置 + 回滚后的选择”，
        // 避免产生用户未做任何编辑却显示未保存的假 dirty；保存期间的后续编辑仍保留。
        if (!disposed) {
          const rollbackActive = persistedActive.value
          const rollbackUrl = ojInstances.value.find((i) => i.id === rollbackActive)?.baseUrl ?? ''
          form.activeOj = rollbackActive
          form.ojUrl = rollbackUrl
          baseline.value = JSON.stringify({ ...submitted, activeOj: rollbackActive, ojUrl: rollbackUrl })
          switchNotice.value = `${target} 实例已保存；切换未成功，可在上方重试`
        }
      }
    } catch (e) {
      if (!disposed) saveError.value = errorMessage(e, '保存配置失败')
    } finally {
      saving.value = false
      saveEpoch += 1
      // 释放等待「旧控制器保存落地」的新控制器 load()
      for (const resolve of saveWaiters.splice(0)) resolve()
    }
  }

  function discard(): void {
    if (baseline.value === '') return
    Object.assign(form, JSON.parse(baseline.value) as SettingsForm)
    saveError.value = null
    switchError.value = null
    switchNotice.value = null
    clearSaved()
  }

  function inputClass(field: string): string {
    return errors.value[field] ? `${INPUT} ${INPUT_ERROR}` : INPUT
  }

  return {
    form,
    dirty,
    saving,
    saveError,
    showSaved,
    errors,
    isValid,
    canSave,
    loading,
    loadError,
    ojOptions,
    switchError,
    switchNotice,
    switching,
    load,
    save,
    discard,
    onSwitchOj,
    inputClass,
  }
}
