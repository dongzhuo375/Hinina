<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import { useRouter } from 'vue-router'
import ErrorMessage from '@/components/common/ErrorMessage.vue'
import LoadingSpinner from '@/components/common/LoadingSpinner.vue'
import { configService } from '@/services/config.service'
import { systemService } from '@/services/system.service'
import { resetSessionForOjSwitch } from '@/stores/session'
import { useAnnouncementStore } from '@/stores/announcementStore'
import { useContestStore } from '@/stores/contestStore'
import { useProblemStore } from '@/stores/problemStore'
import { DEFAULT_LANGUAGES, normalizeHojLanguage } from '@/utils/language'
import {
  EDITOR_FONT_SIZE_MAX,
  EDITOR_FONT_SIZE_MIN,
  EDITOR_TAB_SIZES,
} from '@/utils/editor'
import { errorMessage } from '@/utils/error'
import { ojBaseUrlHint, ojSelectOptions } from '@/utils/oj'
import type { AppConfig, OjInstance } from '@/types/config'
import type { DataDirInfo, LocalDataUsage, StorageInfo } from '@/types/system'

/**
 * 设置页 —— 应用配置（AppConfig）的可视化编辑入口。
 *
 * 数据流：onMounted 先 `invalidate()` 再 `getConfig()`，保证页面展示的恒为磁盘真值
 * （其它页面可能已热改过配置，进程内缓存不可信）；编辑在本地草稿上进行，
 * 保存时经 `configService.updateConfig`（读-改-写整体替换）落盘并失效缓存。
 */

// ── 表单草稿 ──
// 数字字段保留**原始字符串**：输入过程中的中间态（空串/半截数字）不应被强转成
// NaN 写回，只有校验通过的值才会进入保存载荷。

interface SettingsForm {
  /// 当前 OJ 实例 id（下拉选择，候选 = instances 清单）
  activeOj: string
  /// 当前 OJ 实例的服务端地址（保存时写回该实例的 baseUrl）
  ojUrl: string
  /// 当前比赛引用（不透明字符串：HOJ 数字 ID / 其它 OJ 资源引用；空 = 未配置）
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

/// 基线快照（上次加载/保存成功时的表单序列化），用于 dirty 判定与「放弃更改」
const baseline = ref('')

function snapshot(): string {
  return JSON.stringify(form)
}

const dirty = computed(() => baseline.value !== '' && snapshot() !== baseline.value)

// ── 保存状态 ──

const saving = ref(false)
const saveError = ref<string | null>(null)
const showSaved = ref(false)
let savedTimer: ReturnType<typeof setTimeout> | null = null

// ── 校验 ──

/// 严格非负整数解析：拒绝空串/小数/负号/科学计数法等 Number() 会宽容接受的形式
function parseIntStrict(raw: string): number | null {
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

const errors = computed<Record<string, string | null>>(() => ({
  ojUrl: /^https?:\/\/\S+$/i.test(form.ojUrl.trim()) ? null : '须以 http:// 或 https:// 开头',
  // 比赛引用是自由格式字符串（HOJ 数字 / 其它 OJ 资源 ID）；空串 = 未配置，合法
  contestRef: null,
  timeoutSecs: intError(form.timeoutSecs, 1, 120, '请求超时'),
  pollIntervalSecs: intError(form.pollIntervalSecs, 1, 30, '轮询间隔'),
  pollTimeoutSecs: intError(form.pollTimeoutSecs, 30, 3600, '轮询总超时'),
  cacheTtlSecs: intError(form.cacheTtlSecs, 0, 600, '缓存 TTL'),
  fontSize: intError(form.fontSize, EDITOR_FONT_SIZE_MIN, EDITOR_FONT_SIZE_MAX, '字号'),
  autoSaveIntervalSecs: intError(form.autoSaveIntervalSecs, 5, 300, '自动保存间隔'),
}))

const isValid = computed(() => Object.values(errors.value).every((e) => e === null))
const canSave = computed(() => dirty.value && isValid.value && !saving.value)

// ── 加载 ──

const loading = ref(true)
const loadError = ref<string | null>(null)
const router = useRouter()

const TAB_SIZES: readonly number[] = EDITOR_TAB_SIZES
/// 默认语言候选 = HOJ 显示名（值域权威见 utils/language；与提交契约同源）
const LANGUAGE_OPTIONS: readonly string[] = DEFAULT_LANGUAGES

/// OJ 实例清单（populate 时从配置刷新；切换后取新实例地址、渲染禁用态）
const ojInstances = ref<OjInstance[]>([])
/// 下拉候选 = 已知 OJ 枚举（含尚未配置者，选中即引导创建实例）+ 配置里的其它 id
const ojOptions = computed(() => ojSelectOptions(ojInstances.value))
/// 已持久化的当前 OJ（切换失败时回滚下拉显示，保持 UI 与后端一致）
const persistedActive = ref('HOJ')
/// 切换 OJ 的错误提示（独立于保存错误：两个不同意图）
const switchError = ref<string | null>(null)
/// 切换 OJ 的引导提示（尚未配置的 OJ：不是错误，只是告诉用户下一步做什么）
const switchNotice = ref<string | null>(null)

/// 切换的公共尾部：调用后端命令（后端会按需注册新实例）→ 重置会话上下文 → 回登录页。
///
/// 抽出来是因为有两条入口：下拉的显式切换、以及「保存新建实例后自动完成切换」
/// （用户在选下拉时已表达切换意图，只是当时实例还不存在）。失败如实写 `switchError`，
/// 由调用方决定是否回滚下拉显示。
async function performSwitch(target: string): Promise<boolean> {
  try {
    await configService.switchOj(target)
    persistedActive.value = target
    // 旧 OJ 的用户/比赛/题面/提交对新 OJ 全部失效（解题页还会拿旧 contest.id
    // 向新 OJ 提交）：与登出同款清理，但不打后端 logout（Registry 已切换，
    // 那会误删新 OJ 自己的会话文件）
    resetSessionForOjSwitch()
    void router.replace({ name: 'Login' })
    return true
  } catch (e) {
    switchError.value = errorMessage(e, '切换 OJ 失败')
    return false
  }
}

/// 切换当前 OJ：显式命令（即时生效 + 持久化 oj.active + 发布 OJSwitched）。
/// 与「保存」解耦 —— 保存仍负责地址/比赛引用等其余字段。
/// 切换成功 = 整个应用换了服务端：重置会话上下文并回登录页，由路由守卫按新 OJ
/// 的会话文件恢复会话（该 OJ 登录过则无感续用，否则落在登录表单）。
async function onSwitchOj(): Promise<void> {
  switchError.value = null
  switchNotice.value = null
  const target = form.activeOj
  // 尚未配置的 OJ（从枚举里挑出来的新类型）：**不切换**，引导先填地址保存。
  // 直接切必然失败（无实例 → 后端无从注册），把用户丢进一个错误提示不如
  // 明确告诉他下一步：填地址 → 保存（建实例并自动切换）。
  const option = ojOptions.value.find((o) => o.id === target)
  if (option && !option.configured) {
    // 地址栏必须清空：它属于「当前选中的 OJ」，留着旧 OJ 的地址会让「保存」
    // 把它写进新实例（数据损坏）。未保存的地址编辑随之丢弃 —— 与已配置实例的
    // 切换同款语义，但这里显式告知，不静默。
    const discarded = form.ojUrl !== ''
    form.ojUrl = ''
    switchNotice.value =
      `${target} 尚未配置：填写服务器地址后点「保存」，将自动创建实例并切换` +
      (discarded ? '（切换前未保存的地址编辑已丢弃）' : '')
    return
  }
  if (await performSwitch(target)) {
    // 地址栏跟随新实例：否则表单里仍是旧实例的地址，「保存」会把它写进
    // 新实例的 baseUrl（数据损坏）。切换前未保存的地址编辑随之丢弃 ——
    // 用户已切换编辑对象，这是预期行为。
    form.ojUrl = ojInstances.value.find((i) => i.id === target)?.baseUrl ?? ''
  } else {
    // 回滚下拉到已持久化值，避免 UI 停留在一个未生效的 OJ
    form.activeOj = persistedActive.value
  }
}

/// 分栏比例钳位到滑杆值域 [0.30, 0.70]，两位小数（与 step 0.01 对齐）
function clampRatio(value: number): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) return 0.48
  return Math.min(0.7, Math.max(0.3, Math.round(value * 100) / 100))
}

function populate(config: AppConfig): void {
  form.activeOj = config.oj.active
  persistedActive.value = config.oj.active
  switchError.value = null
  switchNotice.value = null
  ojInstances.value = config.oj.instances
  // 展示当前选中实例的地址（active 未命中时回退第一个启用实例，兜底竞态）
  form.ojUrl =
    config.oj.instances.find((i) => i.id === config.oj.active)?.baseUrl
    ?? configService.activeOjBaseUrl(config)
  form.contestRef = config.oj.contestRef ?? ''
  form.contestPassword = config.oj.contestPassword ?? ''
  form.timeoutSecs = String(config.oj.timeoutSecs)
  form.pollIntervalSecs = String(config.oj.pollIntervalSecs)
  form.pollTimeoutSecs = String(config.oj.pollTimeoutSecs)
  form.cacheTtlSecs = String(config.oj.cacheTtlSecs)
  form.cacheProblemStatement = config.oj.cacheProblemStatement ?? true
  form.fontSize = String(config.editor.fontSize)
  form.tabSize = TAB_SIZES.includes(config.editor.tabSize) ? config.editor.tabSize : 4
  form.defaultLanguage = normalizeHojLanguage(config.editor.defaultLanguage)
  form.autoSave = config.editor.autoSave
  form.autoSaveIntervalSecs = String(config.editor.autoSaveIntervalSecs)
  form.splitRatio = clampRatio(config.layout.splitRatio)
  baseline.value = snapshot()
}

async function load(): Promise<void> {
  loading.value = true
  loadError.value = null
  try {
    // 设置页必须展示磁盘真值：先失效进程内缓存再读
    configService.invalidate()
    populate(await configService.getConfig())
  } catch (e) {
    loadError.value = errorMessage(e, '读取配置失败')
  } finally {
    loading.value = false
  }
}

// ── 保存 / 放弃 ──

async function save(): Promise<void> {
  if (!canSave.value) return
  saving.value = true
  saveError.value = null
  // 目标 OJ 与「是否新建实例」在**保存前**定死：两者都用于保存后的自动切换，
  // 绝不能从保存过程中的副作用里读 —— 保存失败时那个标记会残留，导致**下一次
  // 无关保存**去切换一个从未落盘的 OJ（后端必然 ProviderNotFound）。
  const target = form.activeOj
  const isNewInstance = !ojInstances.value.some((i) => i.id === target)
  try {
    // 后端 update_config 是整体替换语义，updateConfig 内部已做读-改-写；
    // 这里只把校验通过的表单值写入草稿
    const saved = await configService.updateConfig((draft) => {
      // active **不在这里写**：它只由 switch_oj 改写（切换是即时生效的独立意图）。
      // 若保存也写 active，新建实例的场景会出现「配置说 Hydro、运行中的 Registry
      // 仍是 HOJ」的静默不一致（保存不触发切换）。
      //
      // 地址写回目标实例；实例不存在则**创建**（设置页支持从枚举里挑一个尚未
      // 配置的 OJ —— 保存即建实例，之后切换无需手改 config.json）
      const instance = draft.oj.instances.find((i) => i.id === target)
      if (instance) {
        instance.baseUrl = form.ojUrl.trim()
        // 保存地址即视为启用该 OJ（否则实例不会注册，切换必失败）
        instance.enabled = true
      } else {
        draft.oj.instances.push({
          id: target,
          baseUrl: form.ojUrl.trim(),
          enabled: true,
          options: {},
        })
      }
      draft.oj.contestRef = form.contestRef.trim()
      draft.oj.contestPassword = form.contestPassword === '' ? null : form.contestPassword
      draft.oj.timeoutSecs = Number(form.timeoutSecs)
      draft.oj.pollIntervalSecs = Number(form.pollIntervalSecs)
      draft.oj.pollTimeoutSecs = Number(form.pollTimeoutSecs)
      draft.oj.cacheTtlSecs = Number(form.cacheTtlSecs)
      draft.oj.cacheProblemStatement = form.cacheProblemStatement
      draft.editor.fontSize = Number(form.fontSize)
      draft.editor.tabSize = form.tabSize
      draft.editor.defaultLanguage = form.defaultLanguage
      draft.editor.autoSave = form.autoSave
      draft.editor.autoSaveIntervalSecs = Number(form.autoSaveIntervalSecs)
      draft.layout.splitRatio = form.splitRatio
    })
    baseline.value = snapshot()
    // 实例清单已变（可能新增了当前 OJ）：同步给下拉，使「configured」状态即时正确 ——
    // 否则刚创建的实例仍被当成未配置，用户重试切换会被拦下
    ojInstances.value = saved.oj.instances
    showSaved.value = true
    if (savedTimer) clearTimeout(savedTimer)
    savedTimer = setTimeout(() => {
      showSaved.value = false
      savedTimer = null
    }, 3_000)

    // 保存时新建了实例（从枚举里挑的未配置 OJ）→ 完成用户**已经表达过**的切换意图：
    // 实例已落盘，后端 `switch_oj` 会按需注册它，无需重启客户端。
    // 失败不吞：写提示让用户在上方重试（此时实例已 configured，重试可成功）。
    if (isNewInstance) {
      if (await performSwitch(target)) return
      switchNotice.value = `${target} 实例已保存；切换未成功，可在上方重试`
    }
  } catch (e) {
    saveError.value = errorMessage(e, '保存配置失败')
  } finally {
    saving.value = false
  }
}

/// 放弃更改：回到基线（上次加载/保存成功时的值）
function discard(): void {
  if (baseline.value === '') return
  Object.assign(form, JSON.parse(baseline.value) as SettingsForm)
  saveError.value = null
}

// ── 关于（存储信息）──

const storage = ref<StorageInfo | null>(null)
const storageFailed = ref(false)

async function loadStorage(): Promise<void> {
  try {
    storage.value = await systemService.getStorageInfo()
  } catch {
    // 非致命：仅「关于」区块降级为「获取失败」
    storageFailed.value = true
  }
}

const copiedKey = ref<string | null>(null)
let copiedTimer: ReturnType<typeof setTimeout> | null = null

async function copyText(key: string, text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text)
    copiedKey.value = key
    if (copiedTimer) clearTimeout(copiedTimer)
    copiedTimer = setTimeout(() => {
      copiedKey.value = null
      copiedTimer = null
    }, 2_000)
  } catch {
    // 剪贴板不可用（权限/环境）时静默忽略，复制不是关键路径
  }
}

// ── 重置与清理 ──
//
// 两个动作的定位刻意分开（入口本身是隐藏的：连点状态栏版本号 5 下才出现设置项）：
// - **重置客户端**：清掉一切可重新从服务端获取的东西。安全、可反复点，故只做一次确认。
// - **清理本地数据**：删除不可重建的本地事实（日志内容、过期提交留档）。不可逆，
//   故必须先展示确切范围与体积、由用户勾选、再二次确认。

const resetting = ref(false)
const resetConfirmOpen = ref(false)
/// 重置结果提示。`warn=true` 表示「重置成功但补拉失败」—— 文案与配色都必须如实，
/// 不能因为重置本身成功就宣称数据已是最新。
const resetNotice = ref<{ text: string; warn: boolean } | null>(null)
const resetError = ref<string | null>(null)

const usage = ref<LocalDataUsage | null>(null)
const usageFailed = ref(false)
const purgeLogs = ref(true)
const purgeSnapshots = ref(true)
const purging = ref(false)
const purgeConfirmOpen = ref(false)
/// 清理结果提示。`warn=true` 表示有勾选项实际没被清掉（如日志文件层不可用）
const purgeNotice = ref<{ text: string; warn: boolean } | null>(null)
const purgeError = ref<string | null>(null)

let resetTimer: ReturnType<typeof setTimeout> | null = null
let purgeTimer: ReturnType<typeof setTimeout> | null = null

/// 文件体积展示（自适应单位）。
///
/// 不复用 `formatCodeLength`：那是「代码长度」口径，恒定按 KB 展示（源码都是 KB 级）；
/// 这里要覆盖字节级留档与 MB 级日志，单位必须自适应。
function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return '-'
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / 1024 / 1024).toFixed(2)} MB`
}

/// 重置客户端，并立刻补拉当前比赛数据。
///
/// **必须补拉**：后端缓存清空后，前端 store 里的内存副本仍是旧值（重置刻意不动
/// 前端状态，避免把界面清成空白）。不补拉的话用户看到的是「已重置」却依旧是旧数据，
/// 等于把「重置是否生效」变成不可验证的玄学。
///
/// 补拉范围只覆盖**可观察的差异**，不追求「全场刷新」：
/// - 比赛元信息 + 题目列表 → `loadContest`（立刻重拉）
/// - 我的题目状态 → `invalidateMyStatus`（标记过期，总览页下次可见时重拉）
/// - 公告列表 + **已读集合** → `refresh`：已读状态已被后端清空，重拉后 readIds
///   归零，红点随之复亮（这正是重置应有的表现）
///
/// 榜单 / 提交历史 / 题面 / limits 的前端副本不补拉：它们与服务端同源，且服务端
/// 本就不缓存这几项，进入对应页面时自然刷新 —— 重置不该变成一次全场请求风暴。
async function resetClient(): Promise<void> {
  resetConfirmOpen.value = false
  resetting.value = true
  resetNotice.value = null
  resetError.value = null
  try {
    await systemService.resetClient()

    // 补拉失败必须如实告知：重置本身已成功，但「数据已重新拉取」这句话不能凭空说
    let refreshed = true
    await useContestStore()
      .loadContest()
      .catch(() => {
        refreshed = false
      })
    useProblemStore().invalidateMyStatus()
    void useAnnouncementStore().refresh()

    resetNotice.value = refreshed
      ? { text: '已重置，数据已重新拉取', warn: false }
      : { text: '已重置，但数据重新拉取失败，请手动刷新', warn: true }
    if (resetTimer) clearTimeout(resetTimer)
    resetTimer = setTimeout(() => {
      resetNotice.value = null
      resetTimer = null
    }, 3_000)
  } catch (e) {
    resetError.value = errorMessage(e, '重置失败')
  } finally {
    resetting.value = false
  }
}

/// 读取可清理项的体积（进入设置页与每次清理后各读一次，保证预览不是陈旧的）
async function loadUsage(): Promise<void> {
  try {
    usage.value = await systemService.localDataUsage()
    usageFailed.value = false
  } catch {
    // 非致命：仅该区块降级为「获取失败」，不影响重置客户端
    usageFailed.value = true
  }
}

/// 清理本地数据（不可逆）。两个勾选项都空时不发请求（后端也如实处理为 no-op）。
async function purgeLocalData(): Promise<void> {
  if (!purgeLogs.value && !purgeSnapshots.value) {
    purgeError.value = '请先选择要清理的内容'
    purgeConfirmOpen.value = false
    return
  }
  purgeConfirmOpen.value = false
  purging.value = true
  purgeNotice.value = null
  purgeError.value = null
  try {
    const report = await systemService.purgeLocalData(purgeLogs.value, purgeSnapshots.value)

    const parts = [`释放 ${formatBytes(report.freedBytes)}`]
    if (report.removedSnapshots > 0) parts.push(`删除留档 ${report.removedSnapshots} 个`)
    // 勾了日志却没清掉（文件层不可用 / 截断失败）必须如实说明 ——
    // 否则界面只剩「释放 0 B」，用户看不出日志其实没被动过
    const logMissed = purgeLogs.value && !report.logCleared
    if (logMissed) parts.push('日志未清理（文件层不可用或写入失败）')

    purgeNotice.value = { text: `已清理：${parts.join('，')}`, warn: logMissed }
    // 清理后重读体积：预览必须反映真值，否则用户会以为没生效
    await loadUsage()
    if (purgeTimer) clearTimeout(purgeTimer)
    purgeTimer = setTimeout(() => {
      purgeNotice.value = null
      purgeTimer = null
    }, 4_000)
  } catch (e) {
    purgeError.value = errorMessage(e, '清理本地数据失败')
  } finally {
    purging.value = false
  }
}

// ── 数据目录 ──
//
// 改动**一律重启后生效**：各 Service 都持有以 base_dir 为根的 Storage、日志还握着
// 文件句柄，运行中热切等于重建整个 AppContext。故这里只「校验 + 记录 + 提示重启」，
// 真正的搬运由下次启动完成（见 Rust `infra::data_dir`）。
//
// 默认目录是 `%LOCALAPPDATA%/{identifier}`（不随域漫游）。之所以要能改：默认落在
// 系统盘，选手可能想放到数据盘；而**回退到临时目录**（`source === 'fallbackTemp'`）
// 意味着数据随时会被系统清理，界面必须显眼告警。

const dataDir = ref<DataDirInfo | null>(null)
const dataDirFailed = ref(false)

/// 选择器返回的候选路径（等待用户确认时才写指针）
const pendingDir = ref<string | null>(null)
/// 是否把现有数据搬到新目录（默认勾选：不迁移会让界面看起来像被重置）
const migrateData = ref(true)
const changingDir = ref(false)
const changeNotice = ref<string | null>(null)
const changeError = ref<string | null>(null)

let changeTimer: ReturnType<typeof setTimeout> | null = null

/// 数据目录来源的中文说明与配色（回退临时目录必须醒目）
const dataDirSourceMeta = computed(() => {
  switch (dataDir.value?.source) {
    case 'custom':
      return { text: '用户指定', cls: 'text-[var(--color-primary)]' }
    case 'fallbackTemp':
      return { text: '临时目录（数据可能被系统清理）', cls: 'text-rose-600' }
    default:
      return { text: '默认位置', cls: 'text-[var(--text-muted)]' }
  }
})

async function loadDataDir(): Promise<void> {
  try {
    dataDir.value = await systemService.getDataDir()
    dataDirFailed.value = false
  } catch {
    // 非致命：仅该区块降级为「获取失败」，不影响其余分组
    dataDirFailed.value = true
  }
}

/// 取消候选目录：**必须复位 `migrateData`**。
///
/// 它是「更改目录」与「恢复默认」共用的状态。若用户在候选面板里取消了勾选后又取消
/// 整个流程，`migrateData` 会停在 `false` —— 接着点「恢复默认」就会**静默不迁移**，
/// 而用户以为自己什么都没改过。故取消即回到默认值。
function cancelDataDirChoice(): void {
  pendingDir.value = null
  migrateData.value = true
}

/// 打开原生目录选择器；取消则什么都不做（不产生任何状态变更）
async function chooseDataDir(): Promise<void> {
  changeError.value = null
  changeNotice.value = null
  try {
    const picked = await systemService.pickDataDir()
    if (!picked) return
    pendingDir.value = picked
    migrateData.value = true
  } catch (e) {
    changeError.value = errorMessage(e, '打开目录选择器失败')
  }
}

/// 确认更改：写位置指针（不搬运，重启后由启动流程完成）
async function confirmDataDir(): Promise<void> {
  const target = pendingDir.value
  if (!target) return
  changingDir.value = true
  changeError.value = null
  try {
    await systemService.setDataDir(target, migrateData.value)
    pendingDir.value = null
    // 复位共用状态：下一次操作（无论「更改目录」还是「恢复默认」）都从默认值开始
    migrateData.value = true
    await loadDataDir()
    changeNotice.value = migrateData.value
      ? '已记录，重启后生效并自动迁移现有数据'
      : '已记录，重启后生效（未迁移现有数据）'
    if (changeTimer) clearTimeout(changeTimer)
    changeTimer = setTimeout(() => {
      changeNotice.value = null
      changeTimer = null
    }, 5_000)
  } catch (e) {
    changeError.value = errorMessage(e, '更改数据目录失败')
  } finally {
    changingDir.value = false
  }
}

/// 恢复默认数据目录（同样重启后生效）
async function restoreDefaultDataDir(): Promise<void> {
  changingDir.value = true
  changeError.value = null
  try {
    await systemService.resetDataDir(migrateData.value)
    pendingDir.value = null
    migrateData.value = true
    await loadDataDir()
    changeNotice.value = '已记录，重启后回到默认数据目录'
    if (changeTimer) clearTimeout(changeTimer)
    changeTimer = setTimeout(() => {
      changeNotice.value = null
      changeTimer = null
    }, 5_000)
  } catch (e) {
    changeError.value = errorMessage(e, '恢复默认目录失败')
  } finally {
    changingDir.value = false
  }
}

// ── 布局滑杆 ──

function onSplitInput(event: Event): void {
  const value = Number((event.target as HTMLInputElement).value)
  if (Number.isFinite(value)) form.splitRatio = clampRatio(value)
}

const splitPercent = computed(() => Math.round(form.splitRatio * 100))

// ── 样式常量 ──

const INPUT =
  'w-full rounded-lg border border-[var(--border-color)] bg-white px-3 py-2 text-sm text-[var(--text-primary)] transition-colors placeholder-[var(--text-muted)] focus:border-[var(--color-primary)] focus:outline-none focus:ring-1 focus:ring-[var(--color-primary)] disabled:cursor-not-allowed disabled:bg-slate-50 disabled:text-[var(--text-muted)]'
const INPUT_ERROR = 'border-rose-400 focus:border-rose-400 focus:ring-rose-400'

function inputClass(field: string): string {
  return errors.value[field] ? `${INPUT} ${INPUT_ERROR}` : INPUT
}

onMounted(() => {
  void load()
  void loadStorage()
  void loadUsage()
  void loadDataDir()
})

onBeforeUnmount(() => {
  if (savedTimer) clearTimeout(savedTimer)
  if (copiedTimer) clearTimeout(copiedTimer)
  if (resetTimer) clearTimeout(resetTimer)
  if (purgeTimer) clearTimeout(purgeTimer)
  if (changeTimer) clearTimeout(changeTimer)
})
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col overflow-hidden bg-[var(--bg-body)]">
    <div class="min-h-0 flex-1 overflow-y-auto">
      <div class="mx-auto w-full max-w-3xl px-6 pb-6 pt-6">
        <!-- 页头 -->
        <header class="mb-5">
          <h1 class="text-lg font-bold tracking-tight text-[var(--text-primary)]">设置</h1>
          <p class="mt-0.5 text-xs text-[var(--text-muted)]">
            配置保存在本地客户端，仅影响当前设备
          </p>
        </header>

        <LoadingSpinner v-if="loading" message="正在加载配置…" />
        <ErrorMessage v-else-if="loadError" :message="loadError" :retry="load" />

        <template v-else>
          <div class="flex flex-col gap-4">
            <!-- ── OJ 服务器 ── -->
            <section class="rounded-xl border border-[var(--border-color)] bg-white shadow-sm">
              <div class="flex items-center gap-2 border-b border-slate-100 px-5 py-3.5">
                <svg
                  class="h-4 w-4 text-[var(--color-primary)]"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  viewBox="0 0 24 24"
                  aria-hidden="true"
                >
                  <rect x="3" y="4" width="18" height="7" rx="1.5" />
                  <rect x="3" y="13" width="18" height="7" rx="1.5" />
                  <path d="M7 7.5h.01M7 16.5h.01" stroke-linecap="round" />
                </svg>
                <h2 class="text-sm font-semibold text-[var(--text-primary)]">OJ 服务器</h2>
              </div>
              <div class="grid grid-cols-1 gap-x-4 gap-y-4 px-5 py-4 sm:grid-cols-2">
                <label class="block">
                  <span class="mb-1 block text-xs font-medium text-[var(--text-secondary)]">
                    当前 OJ
                  </span>
                  <select v-model="form.activeOj" :class="INPUT" @change="onSwitchOj">
                    <option
                      v-for="o in ojOptions"
                      :key="o.id"
                      :value="o.id"
                      :disabled="o.disabled"
                    >
                      {{ o.id }}{{ o.disabled ? '（已禁用）' : o.configured ? '' : '（未配置）' }}
                    </option>
                  </select>
                  <span v-if="switchError" class="mt-1 block text-xs text-rose-600">
                    {{ switchError }}
                  </span>
                  <!-- 引导/安抚提示与错误**并存**：两者语义独立（如「实例已保存但切换未成功」
                       既是错误也需要安抚），用 v-else-if 会让提示永不可见 -->
                  <span v-if="switchNotice" class="mt-1 block text-xs text-amber-600">
                    {{ switchNotice }}
                  </span>
                  <span
                    v-if="!switchError && !switchNotice"
                    class="mt-1 block text-xs text-[var(--text-muted)]"
                  >
                    切换即时生效并持久化，将离开本页并放弃所有未保存的修改；候选 = 已知 OJ 类型 + 配置里的其它实例
                  </span>
                </label>

                <label class="block">
                  <span class="mb-1 block text-xs font-medium text-[var(--text-secondary)]">
                    服务器地址
                  </span>
                  <input
                    v-model="form.ojUrl"
                    type="text"
                    spellcheck="false"
                    :placeholder="ojBaseUrlHint(form.activeOj)"
                    :class="inputClass('ojUrl')"
                  />
                  <span v-if="errors.ojUrl" class="mt-1 block text-xs text-rose-600">
                    {{ errors.ojUrl }}
                  </span>
                  <span v-else class="mt-1 block text-xs text-[var(--text-muted)]">
                    属于当前 OJ（{{ form.activeOj }}）；改动当前 OJ 的地址后需重启生效，新建实例保存后会自动切换
                  </span>
                </label>

                <label class="block">
                  <span class="mb-1 block text-xs font-medium text-[var(--text-secondary)]">
                    比赛 ID / 引用
                  </span>
                  <input
                    v-model="form.contestRef"
                    type="text"
                    spellcheck="false"
                    :class="inputClass('contestRef')"
                  />
                  <span class="mt-1 block text-xs text-[var(--text-muted)]">
                    HOJ 为数字 ID，其它 OJ 为资源引用；留空 = 不自动加载
                  </span>
                </label>

                <label class="block">
                  <span class="mb-1 block text-xs font-medium text-[var(--text-secondary)]">
                    比赛密码
                  </span>
                  <input
                    v-model="form.contestPassword"
                    type="password"
                    autocomplete="off"
                    placeholder="无私有赛密码可留空"
                    :class="INPUT"
                  />
                  <span class="mt-1 block text-xs text-[var(--text-muted)]">
                    仅用于进入私有赛，保存在本地
                  </span>
                </label>

                <label class="block">
                  <span class="mb-1 block text-xs font-medium text-[var(--text-secondary)]">
                    请求超时（秒）
                  </span>
                  <input
                    v-model="form.timeoutSecs"
                    type="text"
                    inputmode="numeric"
                    spellcheck="false"
                    :class="inputClass('timeoutSecs')"
                  />
                  <span v-if="errors.timeoutSecs" class="mt-1 block text-xs text-rose-600">
                    {{ errors.timeoutSecs }}
                  </span>
                  <span v-else class="mt-1 block text-xs text-[var(--text-muted)]">
                    单次 HTTP 请求的超时，1–120
                  </span>
                </label>

                <label class="block">
                  <span class="mb-1 block text-xs font-medium text-[var(--text-secondary)]">
                    轮询间隔（秒）
                  </span>
                  <input
                    v-model="form.pollIntervalSecs"
                    type="text"
                    inputmode="numeric"
                    spellcheck="false"
                    :class="inputClass('pollIntervalSecs')"
                  />
                  <span v-if="errors.pollIntervalSecs" class="mt-1 block text-xs text-rose-600">
                    {{ errors.pollIntervalSecs }}
                  </span>
                  <span v-else class="mt-1 block text-xs text-[var(--text-muted)]">
                    评测结果查询间隔，1–30
                  </span>
                </label>

                <label class="block">
                  <span class="mb-1 block text-xs font-medium text-[var(--text-secondary)]">
                    轮询总超时（秒）
                  </span>
                  <input
                    v-model="form.pollTimeoutSecs"
                    type="text"
                    inputmode="numeric"
                    spellcheck="false"
                    :class="inputClass('pollTimeoutSecs')"
                  />
                  <span v-if="errors.pollTimeoutSecs" class="mt-1 block text-xs text-rose-600">
                    {{ errors.pollTimeoutSecs }}
                  </span>
                  <span v-else class="mt-1 block text-xs text-[var(--text-muted)]">
                    单次提交轮询的总时长上限，30–3600
                  </span>
                </label>

                <label class="block">
                  <span class="mb-1 block text-xs font-medium text-[var(--text-secondary)]">
                    列表缓存 TTL（秒）
                  </span>
                  <input
                    v-model="form.cacheTtlSecs"
                    type="text"
                    inputmode="numeric"
                    spellcheck="false"
                    :class="inputClass('cacheTtlSecs')"
                  />
                  <span v-if="errors.cacheTtlSecs" class="mt-1 block text-xs text-rose-600">
                    {{ errors.cacheTtlSecs }}
                  </span>
                  <span v-else class="mt-1 block text-xs text-[var(--text-muted)]">
                    0 表示禁用缓存，0–600
                  </span>
                </label>

                <div class="block">
                  <span class="mb-1 block text-xs font-medium text-[var(--text-secondary)]">
                    题面缓存
                  </span>
                  <div class="flex items-center gap-3 py-1">
                    <button
                      type="button"
                      role="switch"
                      :aria-checked="form.cacheProblemStatement"
                      class="relative h-6 w-11 shrink-0 rounded-full transition-colors"
                      :class="
                        form.cacheProblemStatement ? 'bg-[var(--color-primary)]' : 'bg-slate-300'
                      "
                      @click="form.cacheProblemStatement = !form.cacheProblemStatement"
                    >
                      <span
                        class="absolute top-0.5 left-0.5 h-5 w-5 rounded-full bg-white shadow transition-transform"
                        :class="form.cacheProblemStatement ? 'translate-x-5' : ''"
                      ></span>
                    </button>
                    <span class="text-xs text-[var(--text-secondary)]">
                      {{ form.cacheProblemStatement ? '已开启' : '已关闭' }}
                    </span>
                  </div>
                  <span class="mt-1 block text-xs text-[var(--text-muted)]">
                    缓存题面（内存 + 磁盘），切题来回与断网时秒开；关闭后每次打开都请求服务端
                  </span>
                </div>
              </div>
            </section>

            <!-- ── 编辑器 ── -->
            <section class="rounded-xl border border-[var(--border-color)] bg-white shadow-sm">
              <div class="flex items-center gap-2 border-b border-slate-100 px-5 py-3.5">
                <svg
                  class="h-4 w-4 text-[var(--color-primary)]"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  viewBox="0 0 24 24"
                  aria-hidden="true"
                >
                  <path
                    d="m8 9-3 3 3 3m8-6 3 3-3 3M13.5 6l-3 12"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  />
                </svg>
                <h2 class="text-sm font-semibold text-[var(--text-primary)]">编辑器</h2>
                <span class="ml-auto text-xs text-[var(--text-muted)]">
                  解题页编辑器设置可即时调整，此处改动对新打开的解题页生效
                </span>
              </div>
              <div class="grid grid-cols-1 gap-x-4 gap-y-4 px-5 py-4 sm:grid-cols-2">
                <label class="block">
                  <span class="mb-1 block text-xs font-medium text-[var(--text-secondary)]">
                    字号
                  </span>
                  <input
                    v-model="form.fontSize"
                    type="text"
                    inputmode="numeric"
                    spellcheck="false"
                    :class="inputClass('fontSize')"
                  />
                  <span v-if="errors.fontSize" class="mt-1 block text-xs text-rose-600">
                    {{ errors.fontSize }}
                  </span>
                  <span v-else class="mt-1 block text-xs text-[var(--text-muted)]">8–32</span>
                </label>

                <label class="block">
                  <span class="mb-1 block text-xs font-medium text-[var(--text-secondary)]">
                    Tab 宽度
                  </span>
                  <select v-model.number="form.tabSize" :class="INPUT">
                    <option v-for="size in TAB_SIZES" :key="size" :value="size">
                      {{ size }} 空格
                    </option>
                  </select>
                  <span class="mt-1 block text-xs text-[var(--text-muted)]">
                    缩进宽度（ICPC 惯例 4）
                  </span>
                </label>

                <label class="block">
                  <span class="mb-1 block text-xs font-medium text-[var(--text-secondary)]">
                    默认语言
                  </span>
                  <select v-model="form.defaultLanguage" :class="INPUT">
                    <option v-for="lang in LANGUAGE_OPTIONS" :key="lang" :value="lang">
                      {{ lang }}
                    </option>
                  </select>
                  <span class="mt-1 block text-xs text-[var(--text-muted)]">
                    新建代码文件时的初始语言
                  </span>
                </label>

                <div class="block">
                  <span class="mb-1 block text-xs font-medium text-[var(--text-secondary)]">
                    自动保存
                  </span>
                  <div class="flex items-center gap-3 py-1">
                    <button
                      type="button"
                      role="switch"
                      :aria-checked="form.autoSave"
                      class="relative h-6 w-11 shrink-0 rounded-full transition-colors"
                      :class="form.autoSave ? 'bg-[var(--color-primary)]' : 'bg-slate-300'"
                      @click="form.autoSave = !form.autoSave"
                    >
                      <span
                        class="absolute left-0.5 top-0.5 h-5 w-5 rounded-full bg-white shadow transition-transform"
                        :class="form.autoSave ? 'translate-x-5' : ''"
                      ></span>
                    </button>
                    <span class="text-xs text-[var(--text-secondary)]">
                      {{ form.autoSave ? '已开启' : '已关闭' }}
                    </span>
                  </div>
                  <span class="mt-1 block text-xs text-[var(--text-muted)]">
                    定时保存解题页代码，崩溃后可恢复；关闭后仅在切题、失焦或关窗时落盘
                  </span>
                </div>

                <label class="block">
                  <span
                    class="mb-1 block text-xs font-medium"
                    :class="
                      form.autoSave ? 'text-[var(--text-secondary)]' : 'text-[var(--text-muted)]'
                    "
                  >
                    自动保存间隔（秒）
                  </span>
                  <input
                    v-model="form.autoSaveIntervalSecs"
                    type="text"
                    inputmode="numeric"
                    spellcheck="false"
                    :disabled="!form.autoSave"
                    :class="inputClass('autoSaveIntervalSecs')"
                  />
                  <span
                    v-if="errors.autoSaveIntervalSecs"
                    class="mt-1 block text-xs text-rose-600"
                  >
                    {{ errors.autoSaveIntervalSecs }}
                  </span>
                  <span v-else class="mt-1 block text-xs text-[var(--text-muted)]">5–300</span>
                </label>
              </div>
            </section>

            <!-- ── 布局 ── -->
            <section class="rounded-xl border border-[var(--border-color)] bg-white shadow-sm">
              <div class="flex items-center gap-2 border-b border-slate-100 px-5 py-3.5">
                <svg
                  class="h-4 w-4 text-[var(--color-primary)]"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  viewBox="0 0 24 24"
                  aria-hidden="true"
                >
                  <rect x="3" y="4" width="18" height="16" rx="1.5" />
                  <path d="M10 4v16" />
                </svg>
                <h2 class="text-sm font-semibold text-[var(--text-primary)]">布局</h2>
              </div>
              <div class="px-5 py-4">
                <div class="mb-2 flex items-center justify-between">
                  <span class="text-xs font-medium text-[var(--text-secondary)]">
                    解题页分栏比例
                  </span>
                  <span class="font-mono text-xs font-semibold text-[var(--color-primary)]">
                    左栏 {{ splitPercent }}%
                  </span>
                </div>
                <input
                  type="range"
                  min="0.3"
                  max="0.7"
                  step="0.01"
                  :value="form.splitRatio"
                  class="w-full accent-[var(--color-primary)]"
                  @input="onSplitInput"
                />
                <div
                  class="mt-1 flex justify-between font-mono text-[10px] text-[var(--text-muted)]"
                >
                  <span>30%</span>
                  <span>70%</span>
                </div>
                <p class="mt-2 text-xs text-[var(--text-muted)]">
                  拖拽解题页分栏条也会自动保存该比例
                </p>
              </div>
            </section>

            <!-- ── 主题 ── -->
            <section class="rounded-xl border border-[var(--border-color)] bg-white shadow-sm">
              <div class="flex items-center gap-2 border-b border-slate-100 px-5 py-3.5">
                <svg
                  class="h-4 w-4 text-[var(--color-primary)]"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  viewBox="0 0 24 24"
                  aria-hidden="true"
                >
                  <circle cx="12" cy="12" r="4" />
                  <path
                    d="M12 2v2m0 16v2M4.9 4.9l1.4 1.4m11.4 11.4 1.4 1.4M2 12h2m16 0h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"
                    stroke-linecap="round"
                  />
                </svg>
                <h2 class="text-sm font-semibold text-[var(--text-primary)]">主题</h2>
              </div>
              <div class="grid grid-cols-1 gap-x-4 gap-y-4 px-5 py-4 sm:grid-cols-2">
                <label class="block">
                  <span class="mb-1 block text-xs font-medium text-[var(--text-secondary)]">
                    界面主题
                  </span>
                  <select disabled :class="INPUT">
                    <option>浅色</option>
                  </select>
                </label>
                <label class="block">
                  <span class="mb-1 block text-xs font-medium text-[var(--text-secondary)]">
                    编辑器主题
                  </span>
                  <select disabled :class="INPUT">
                    <option>由解题页「编辑器设置」控制</option>
                  </select>
                </label>
                <p class="text-xs text-[var(--text-muted)] sm:col-span-2">
                  界面暗色主题即将上线，当前版本固定浅色；编辑器主题在解题页「编辑器设置」中即时切换
                </p>
              </div>
            </section>

            <!-- ── 重置与清理 ── -->
            <section class="rounded-xl border border-[var(--border-color)] bg-white shadow-sm">
              <div class="flex items-center gap-2 border-b border-slate-100 px-5 py-3.5">
                <svg
                  class="h-4 w-4 text-[var(--color-primary)]"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  viewBox="0 0 24 24"
                  aria-hidden="true"
                >
                  <ellipse cx="12" cy="5" rx="8" ry="3" />
                  <path
                    d="M4 5v6c0 1.66 3.58 3 8 3s8-1.34 8-3V5M4 11v6c0 1.66 3.58 3 8 3s8-1.34 8-3v-6"
                    stroke-linecap="round"
                  />
                </svg>
                <h2 class="text-sm font-semibold text-[var(--text-primary)]">重置与清理</h2>
              </div>

              <div class="divide-y divide-slate-100">
                <!-- 重置客户端（安全，可反复点） -->
                <div class="px-5 py-4">
                  <div class="flex flex-wrap items-start justify-between gap-3">
                    <p class="min-w-0 flex-1 text-xs leading-relaxed text-[var(--text-secondary)]">
                      <span class="font-medium text-[var(--text-primary)]">重置客户端</span>
                      —— 清空比赛元信息、题面、题目限制、终态提交详情等本地缓存，并清空公告已读标记，清完立即重新拉取。
                      <span class="text-[var(--text-muted)]">
                        不会删除工作区代码、提交源码留档、登录会话与配置。
                      </span>
                    </p>

                    <div class="flex shrink-0 items-center gap-2">
                      <span v-if="resetError" class="text-xs text-rose-600">{{ resetError }}</span>
                      <span
                        v-else-if="resetNotice"
                        class="text-xs"
                        :class="resetNotice.warn ? 'text-amber-600' : 'text-emerald-600'"
                      >
                        {{ resetNotice.text }}
                      </span>

                      <template v-if="resetConfirmOpen || resetting">
                        <button
                          v-if="!resetting"
                          type="button"
                          class="rounded-lg border border-[var(--border-color)] px-3 py-1.5 text-xs font-medium text-[var(--text-secondary)] transition-colors hover:bg-slate-50"
                          @click="resetConfirmOpen = false"
                        >
                          取消
                        </button>
                        <button
                          type="button"
                          class="rounded-lg bg-[var(--color-primary)] px-3 py-1.5 text-xs font-semibold text-white transition-colors hover:brightness-110 disabled:opacity-60"
                          :disabled="resetting"
                          @click="resetClient"
                        >
                          {{ resetting ? '重置中…' : '确认重置' }}
                        </button>
                      </template>
                      <button
                        v-else
                        type="button"
                        class="rounded-lg border border-[var(--border-color)] px-3 py-1.5 text-xs font-medium text-[var(--text-primary)] transition-colors hover:bg-slate-50"
                        @click="resetConfirmOpen = true"
                      >
                        重置客户端
                      </button>
                    </div>
                  </div>
                </div>

                <!-- 清理本地数据（不可逆：先看范围与体积，再勾选确认） -->
                <div class="px-5 py-4">
                  <p class="text-xs leading-relaxed text-[var(--text-secondary)]">
                    <span class="font-medium text-[var(--text-primary)]">清理本地数据</span>
                    —— 删除<strong class="font-semibold text-rose-600">不可重建</strong>的本地内容。
                    日志是排障线索，提交源码留档是 OJ 不回吐代码时的唯一来源，删掉都无法恢复。
                  </p>

                  <!-- 体积预览：不可逆动作必须让用户看到确切范围 -->
                  <div
                    class="mt-3 flex flex-col gap-2 rounded-lg border border-[var(--border-color)] bg-slate-50/70 px-3.5 py-3"
                  >
                    <p v-if="usageFailed" class="text-xs text-rose-600">占用信息获取失败</p>
                    <p v-else-if="!usage" class="text-xs text-[var(--text-muted)]">正在统计占用…</p>
                    <template v-else>
                      <label class="flex items-start gap-2 text-xs text-[var(--text-secondary)]">
                        <input
                          v-model="purgeLogs"
                          type="checkbox"
                          class="mt-0.5 h-3.5 w-3.5 shrink-0 accent-[var(--color-primary)]"
                        />
                        <span class="min-w-0">
                          日志内容
                          <span class="font-mono text-[var(--text-primary)]">
                            {{ formatBytes(usage.logBytes) }}
                          </span>
                          <span class="text-[var(--text-muted)]">
                            （只清空内容，保留日志文件本身）
                          </span>
                        </span>
                      </label>
                      <label class="flex items-start gap-2 text-xs text-[var(--text-secondary)]">
                        <input
                          v-model="purgeSnapshots"
                          type="checkbox"
                          class="mt-0.5 h-3.5 w-3.5 shrink-0 accent-[var(--color-primary)]"
                        />
                        <span class="min-w-0">
                          {{ usage.keepDays }} 天前的提交源码留档
                          <span class="font-mono text-[var(--text-primary)]">
                            {{ usage.snapshotStaleCount }} 个 · {{ formatBytes(usage.snapshotStaleBytes) }}
                          </span>
                          <span class="text-[var(--text-muted)]">
                            （留档共 {{ usage.snapshotTotalCount }} 个 ·
                            {{ formatBytes(usage.snapshotTotalBytes) }}，近 {{ usage.keepDays }} 天的不受影响）
                          </span>
                        </span>
                      </label>
                    </template>
                  </div>

                  <div class="mt-3 flex flex-wrap items-center justify-between gap-3">
                    <p class="min-w-0 flex-1 text-[11px] leading-relaxed text-[var(--text-muted)]">
                      此操作<strong class="font-semibold text-rose-600">不可撤销</strong>；
                      不会删除工作区代码、配置、登录会话与公告已读状态。
                    </p>
                    <div class="flex shrink-0 items-center gap-2">
                      <span v-if="purgeError" class="text-xs text-rose-600">{{ purgeError }}</span>
                      <span
                        v-else-if="purgeNotice"
                        class="text-xs"
                        :class="purgeNotice.warn ? 'text-amber-600' : 'text-emerald-600'"
                      >
                        {{ purgeNotice.text }}
                      </span>

                      <template v-if="purgeConfirmOpen || purging">
                        <button
                          v-if="!purging"
                          type="button"
                          class="rounded-lg border border-[var(--border-color)] px-3 py-1.5 text-xs font-medium text-[var(--text-secondary)] transition-colors hover:bg-slate-50"
                          @click="purgeConfirmOpen = false"
                        >
                          取消
                        </button>
                        <button
                          type="button"
                          class="rounded-lg bg-rose-600 px-3 py-1.5 text-xs font-semibold text-white transition-colors hover:bg-rose-700 disabled:opacity-60"
                          :disabled="purging"
                          @click="purgeLocalData"
                        >
                          {{ purging ? '清理中…' : '确认清理（不可恢复）' }}
                        </button>
                      </template>
                      <button
                        v-else
                        type="button"
                        class="rounded-lg border border-[var(--border-color)] px-3 py-1.5 text-xs font-medium text-[var(--text-primary)] transition-colors hover:bg-slate-50"
                        @click="purgeConfirmOpen = true"
                      >
                        清理本地数据
                      </button>
                    </div>
                  </div>
                </div>
              </div>
            </section>

            <!-- ── 数据目录 ── -->
            <section class="rounded-xl border border-[var(--border-color)] bg-white shadow-sm">
              <div class="flex items-center gap-2 border-b border-slate-100 px-5 py-3.5">
                <svg
                  class="h-4 w-4 text-[var(--color-primary)]"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  viewBox="0 0 24 24"
                  aria-hidden="true"
                >
                  <path
                    d="M3 7a2 2 0 012-2h4l2 2h8a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2V7z"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  />
                </svg>
                <h2 class="text-sm font-semibold text-[var(--text-primary)]">数据目录</h2>
              </div>

              <div class="flex flex-col gap-3 px-5 py-4">
                <p v-if="dataDirFailed" class="text-xs text-rose-600">数据目录信息获取失败</p>
                <p v-else-if="!dataDir" class="text-xs text-[var(--text-muted)]">正在读取…</p>
                <template v-else>
                  <div class="text-xs leading-relaxed text-[var(--text-secondary)]">
                    <div class="flex items-center gap-2">
                      <span class="shrink-0 text-[var(--text-muted)]">当前</span>
                      <span
                        class="min-w-0 flex-1 truncate font-mono text-[var(--text-primary)]"
                        :title="dataDir.currentDir"
                      >
                        {{ dataDir.currentDir }}
                      </span>
                      <span class="shrink-0 font-medium" :class="dataDirSourceMeta.cls">
                        {{ dataDirSourceMeta.text }}
                      </span>
                    </div>
                    <div
                      v-if="dataDir.source === 'custom'"
                      class="mt-1 flex items-center gap-2 text-[var(--text-muted)]"
                    >
                      <span class="shrink-0">默认</span>
                      <span class="min-w-0 flex-1 truncate font-mono" :title="dataDir.defaultDir">
                        {{ dataDir.defaultDir }}
                      </span>
                    </div>
                  </div>

                  <!-- 回退临时目录必须显眼：这正是本次要消除的状态 -->
                  <p
                    v-if="dataDir.source === 'fallbackTemp'"
                    class="rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-xs leading-relaxed text-rose-700"
                  >
                    默认数据目录不可用，当前数据存在<strong class="font-semibold">临时目录</strong>中
                    —— 系统清理临时文件时会连带删除你的代码与提交留档，请尽快更改数据目录。
                  </p>

                  <!-- 待重启提示：改动已记录但尚未生效，必须说清「现在仍在用旧目录」 -->
                  <p
                    v-if="dataDir.restartRequired"
                    class="rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-xs leading-relaxed text-amber-700"
                  >
                    已记录目录更改，<strong class="font-semibold">重启客户端后生效</strong>；
                    当前仍在使用上面的目录。
                  </p>

                  <!-- 选择器已返回候选路径：确认 + 是否迁移 -->
                  <div
                    v-if="pendingDir"
                    class="flex flex-col gap-2 rounded-lg border border-[var(--border-color)] bg-slate-50/70 px-3.5 py-3"
                  >
                    <div class="flex items-center gap-2 text-xs">
                      <span class="shrink-0 text-[var(--text-muted)]">新目录</span>
                      <span
                        class="min-w-0 flex-1 truncate font-mono text-[var(--text-primary)]"
                        :title="pendingDir"
                      >
                        {{ pendingDir }}
                      </span>
                    </div>
                    <label class="flex items-start gap-2 text-xs text-[var(--text-secondary)]">
                      <input
                        v-model="migrateData"
                        type="checkbox"
                        class="mt-0.5 h-3.5 w-3.5 shrink-0 accent-[var(--color-primary)]"
                      />
                      <span>
                        把现有数据迁移到新目录（工作区代码、提交留档、配置、会话）
                        <span class="text-[var(--text-muted)]">
                          —— 不勾选则新目录从空开始，旧数据原地保留
                        </span>
                      </span>
                    </label>
                  </div>

                  <div class="flex flex-wrap items-center justify-between gap-3">
                    <p class="min-w-0 flex-1 text-[11px] leading-relaxed text-[var(--text-muted)]">
                      数据目录决定工作区代码、提交留档、配置与日志的存放位置。
                      更改后需重启客户端生效。
                    </p>
                    <div class="flex shrink-0 items-center gap-2">
                      <span v-if="changeError" class="text-xs text-rose-600">{{ changeError }}</span>
                      <span v-else-if="changeNotice" class="text-xs text-amber-600">
                        {{ changeNotice }}
                      </span>

                      <template v-if="pendingDir">
                        <button
                          type="button"
                          class="rounded-lg border border-[var(--border-color)] px-3 py-1.5 text-xs font-medium text-[var(--text-secondary)] transition-colors hover:bg-slate-50"
                          :disabled="changingDir"
                          @click="cancelDataDirChoice"
                        >
                          取消
                        </button>
                        <button
                          type="button"
                          class="rounded-lg bg-[var(--color-primary)] px-3 py-1.5 text-xs font-semibold text-white transition-colors hover:brightness-110 disabled:opacity-60"
                          :disabled="changingDir"
                          @click="confirmDataDir"
                        >
                          {{ changingDir ? '记录中…' : '确认更改' }}
                        </button>
                      </template>
                      <template v-else>
                        <button
                          v-if="dataDir.source === 'custom'"
                          type="button"
                          class="rounded-lg border border-[var(--border-color)] px-3 py-1.5 text-xs font-medium text-[var(--text-secondary)] transition-colors hover:bg-slate-50 disabled:opacity-60"
                          :disabled="changingDir"
                          @click="restoreDefaultDataDir"
                        >
                          恢复默认
                        </button>
                        <button
                          type="button"
                          class="rounded-lg border border-[var(--border-color)] px-3 py-1.5 text-xs font-medium text-[var(--text-primary)] transition-colors hover:bg-slate-50 disabled:opacity-60"
                          :disabled="changingDir"
                          @click="chooseDataDir"
                        >
                          更改目录…
                        </button>
                      </template>
                    </div>
                  </div>
                </template>
              </div>
            </section>

            <!-- ── 关于 ── -->
            <section class="rounded-xl border border-[var(--border-color)] bg-white shadow-sm">
              <div class="flex items-center gap-2 border-b border-slate-100 px-5 py-3.5">
                <svg
                  class="h-4 w-4 text-[var(--color-primary)]"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  viewBox="0 0 24 24"
                  aria-hidden="true"
                >
                  <circle cx="12" cy="12" r="9" />
                  <path d="M12 11v5m0-8h.01" stroke-linecap="round" />
                </svg>
                <h2 class="text-sm font-semibold text-[var(--text-primary)]">关于</h2>
              </div>
              <dl class="divide-y divide-slate-100 px-5">
                <template v-if="storage">
                  <div class="flex items-center gap-3 py-3">
                    <dt class="w-24 shrink-0 text-xs font-medium text-[var(--text-secondary)]">
                      客户端版本
                    </dt>
                    <dd
                      class="min-w-0 flex-1 truncate font-mono text-xs text-[var(--text-primary)]"
                    >
                      {{ storage.version }}
                    </dd>
                    <button
                      type="button"
                      class="shrink-0 rounded-md border border-[var(--border-color)] px-2 py-0.5 text-xs text-[var(--text-secondary)] transition-colors hover:bg-slate-50"
                      @click="copyText('version', storage.version)"
                    >
                      {{ copiedKey === 'version' ? '已复制' : '复制' }}
                    </button>
                  </div>
                  <div class="flex items-center gap-3 py-3">
                    <dt class="w-24 shrink-0 text-xs font-medium text-[var(--text-secondary)]">
                      存储目录
                    </dt>
                    <dd
                      class="min-w-0 flex-1 truncate font-mono text-xs text-[var(--text-primary)]"
                      :title="storage.baseDir"
                    >
                      {{ storage.baseDir }}
                    </dd>
                    <button
                      type="button"
                      class="shrink-0 rounded-md border border-[var(--border-color)] px-2 py-0.5 text-xs text-[var(--text-secondary)] transition-colors hover:bg-slate-50"
                      @click="copyText('baseDir', storage.baseDir)"
                    >
                      {{ copiedKey === 'baseDir' ? '已复制' : '复制' }}
                    </button>
                  </div>
                  <div class="flex items-center gap-3 py-3">
                    <dt class="w-24 shrink-0 text-xs font-medium text-[var(--text-secondary)]">
                      日志文件
                    </dt>
                    <dd
                      class="min-w-0 flex-1 truncate font-mono text-xs text-[var(--text-primary)]"
                      :title="storage.logPath"
                    >
                      {{ storage.logPath }}
                    </dd>
                    <button
                      type="button"
                      class="shrink-0 rounded-md border border-[var(--border-color)] px-2 py-0.5 text-xs text-[var(--text-secondary)] transition-colors hover:bg-slate-50"
                      @click="copyText('logPath', storage.logPath)"
                    >
                      {{ copiedKey === 'logPath' ? '已复制' : '复制' }}
                    </button>
                  </div>
                </template>
                <div v-else class="flex items-center gap-3 py-3">
                  <dt class="w-24 shrink-0 text-xs font-medium text-[var(--text-secondary)]">
                    存储信息
                  </dt>
                  <dd
                    class="flex-1 text-xs"
                    :class="storageFailed ? 'text-rose-600' : 'text-[var(--text-muted)]'"
                  >
                    {{ storageFailed ? '获取失败' : '正在获取…' }}
                  </dd>
                </div>
              </dl>
            </section>
          </div>

          <!-- 底部粘性操作栏 -->
          <div
            class="sticky bottom-0 z-10 mt-4 rounded-xl border border-[var(--border-color)] bg-white/95 px-4 py-3 shadow-lg backdrop-blur"
          >
            <div
              v-if="saveError"
              class="mb-2 rounded-lg border border-[var(--color-wa-border)] bg-[var(--color-wa-bg)] px-3 py-2 text-xs text-rose-700"
            >
              保存失败：{{ saveError }}
            </div>
            <div class="flex items-center justify-between gap-3">
              <div class="min-w-0 text-xs">
                <span
                  v-if="showSaved"
                  class="inline-flex items-center gap-1.5 rounded-full border border-emerald-200 bg-emerald-50 px-2.5 py-1 font-medium text-emerald-700"
                >
                  <svg
                    class="h-3 w-3"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="3"
                    viewBox="0 0 24 24"
                    aria-hidden="true"
                  >
                    <path d="m5 13 4 4L19 7" stroke-linecap="round" stroke-linejoin="round" />
                  </svg>
                  已保存
                </span>
                <span v-else-if="dirty && !isValid" class="text-rose-600">
                  存在校验错误，请修正后再保存
                </span>
                <span v-else-if="dirty" class="text-amber-600">有未保存的更改</span>
                <span v-else class="text-[var(--text-muted)]">所有更改已保存</span>
              </div>
              <div class="flex shrink-0 items-center gap-2">
                <button
                  type="button"
                  class="rounded-lg border border-[var(--border-color)] bg-white px-4 py-1.5 text-sm text-[var(--text-secondary)] transition-colors enabled:hover:bg-slate-50 disabled:cursor-not-allowed disabled:opacity-40"
                  :disabled="!dirty || saving"
                  @click="discard"
                >
                  放弃更改
                </button>
                <button
                  type="button"
                  class="rounded-lg bg-[var(--color-primary)] px-4 py-1.5 text-sm font-medium text-white transition-colors enabled:hover:bg-[var(--color-primary-hover)] disabled:cursor-not-allowed disabled:opacity-40"
                  :disabled="!canSave"
                  @click="save"
                >
                  {{ saving ? '保存中…' : '保存' }}
                </button>
              </div>
            </div>
          </div>
        </template>
      </div>
    </div>
  </div>
</template>
