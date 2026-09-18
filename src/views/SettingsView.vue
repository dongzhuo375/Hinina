<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import { useRouter } from 'vue-router'
import ErrorMessage from '@/components/common/ErrorMessage.vue'
import LoadingSpinner from '@/components/common/LoadingSpinner.vue'
import { configService } from '@/services/config.service'
import { systemService } from '@/services/system.service'
import { resetSessionForOjSwitch } from '@/stores/session'
import { DEFAULT_LANGUAGES, normalizeHojLanguage } from '@/utils/language'
import {
  EDITOR_FONT_SIZE_MAX,
  EDITOR_FONT_SIZE_MIN,
  EDITOR_TAB_SIZES,
} from '@/utils/editor'
import { errorMessage } from '@/utils/error'
import type { AppConfig, OjInstance } from '@/types/config'
import type { StorageInfo } from '@/types/system'

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
/// 下拉候选：全部实例（禁用者标注且不可选 —— 切到未注册的 OJ 只会得到
/// ProviderNotFound；若当前 active 恰为禁用实例，仍如实显示为选中值）
const ojOptions = computed(() =>
  ojInstances.value.map((i) => ({ id: i.id, disabled: !i.enabled })),
)
/// 已持久化的当前 OJ（切换失败时回滚下拉显示，保持 UI 与后端一致）
const persistedActive = ref('HOJ')
/// 切换 OJ 的错误提示（独立于保存错误：两个不同意图）
const switchError = ref<string | null>(null)

/// 切换当前 OJ：显式命令（即时生效 + 持久化 oj.active + 发布 OJSwitched）。
/// 与「保存」解耦 —— 保存仍负责地址/比赛引用等其余字段（active 两处写入同源同值）。
/// 切换成功 = 整个应用换了服务端：重置会话上下文并回登录页，由路由守卫按新 OJ
/// 的会话文件恢复会话（该 OJ 登录过则无感续用，否则落在登录表单）。
async function onSwitchOj(): Promise<void> {
  switchError.value = null
  const target = form.activeOj
  try {
    await configService.switchOj(target)
    persistedActive.value = target
    // 地址栏跟随新实例：否则表单里仍是旧实例的地址，「保存」会把它写进
    // 新实例的 baseUrl（数据损坏）。切换前未保存的地址编辑随之丢弃 ——
    // 用户已切换编辑对象，这是预期行为。
    form.ojUrl = ojInstances.value.find((i) => i.id === target)?.baseUrl ?? ''
    // 旧 OJ 的用户/比赛/题面/提交对新 OJ 全部失效（解题页还会拿旧 contest.id
    // 向新 OJ 提交）：与登出同款清理，但不打后端 logout（Registry 已切换，
    // 那会误删新 OJ 自己的会话文件）
    resetSessionForOjSwitch()
    void router.replace({ name: 'Login' })
  } catch (e) {
    // 回滚下拉到已持久化值，避免 UI 停留在一个未生效的 OJ
    form.activeOj = persistedActive.value
    switchError.value = errorMessage(e, '切换 OJ 失败')
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
  try {
    // 后端 update_config 是整体替换语义，updateConfig 内部已做读-改-写；
    // 这里只把校验通过的表单值写入草稿
    await configService.updateConfig((draft) => {
      draft.oj.active = form.activeOj
      // 地址写回当前选中实例（active 不在实例列表会被 Rust validate 拒绝，
      // 下拉候选即实例清单，正常操作不会出现）
      const instance = draft.oj.instances.find((i) => i.id === form.activeOj)
      if (instance) instance.baseUrl = form.ojUrl.trim()
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
    showSaved.value = true
    if (savedTimer) clearTimeout(savedTimer)
    savedTimer = setTimeout(() => {
      showSaved.value = false
      savedTimer = null
    }, 3_000)
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
})

onBeforeUnmount(() => {
  if (savedTimer) clearTimeout(savedTimer)
  if (copiedTimer) clearTimeout(copiedTimer)
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
                      {{ o.id }}{{ o.disabled ? '（已禁用）' : '' }}
                    </option>
                  </select>
                  <span v-if="switchError" class="mt-1 block text-xs text-rose-600">
                    {{ switchError }}
                  </span>
                  <span v-else class="mt-1 block text-xs text-[var(--text-muted)]">
                    切换即时生效并持久化；候选 = 配置文件 oj.instances 清单
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
                    placeholder="https://example-oj.com"
                    :class="inputClass('ojUrl')"
                  />
                  <span v-if="errors.ojUrl" class="mt-1 block text-xs text-rose-600">
                    {{ errors.ojUrl }}
                  </span>
                  <span v-else class="mt-1 block text-xs text-[var(--text-muted)]">
                    修改后需重启客户端生效
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
                    定时保存解题页代码，崩溃后可恢复
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
