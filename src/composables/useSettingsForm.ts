import { computed, reactive, ref } from 'vue'
import type { Ref } from 'vue'
import { useTransientValue } from '@/composables/useTransientValue'
import { useRouter } from 'vue-router'
import { configService } from '@/services/config.service'
import { systemService } from '@/services/system.service'
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
  // 存量值不得阻塞可编辑字段的保存**（评审十 #3：手改越界 config.json 时字体/
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

  const saving = ref(false)
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
  const canSave = computed(() => dirty.value && isValid.value && !saving.value)

  const loading = ref(true)
  const loadError = ref<string | null>(null)
  const router = useRouter()

  const ojInstances = ref<OjInstance[]>([])
  const ojOptions = computed(() => ojSelectOptions(ojInstances.value))
  const persistedActive = ref('HOJ')
  const switchError = ref<string | null>(null)
  const switchNotice = ref<string | null>(null)

  async function performSwitch(target: string): Promise<boolean> {
    try {
      await configService.switchOj(target)
      persistedActive.value = target
      resetSessionForOjSwitch()
      // `--settings`（管理方钥匙）：切换成功**留在设置页**继续修配置——踢到
      // 登录页等于重新锁死（firstNavigation 已消费、无 UI 回入口）。会话已由
      // resetSessionForOjSwitch 清理，最小壳接管（内容区共享实例，草稿保留）。
      // 第三处豁免导航源，与 sessionGuard/阶段闸门同理由（评审八 P2）。
      // **豁免只属于设置页本身**：switch_oj 是异步的，期间用户可能已导航到
      // 比赛页——不查当前路由就把「停留」带过去，无会话用户会被留在比赛页
      //（评审九 P2-1）。判据与 sessionGuard 一致：钥匙 + 当前路由是设置页。
      const stayOnSettings = systemService.isKeyedSettings(router.currentRoute.value?.name)
      if (!stayOnSettings) {
        void router.replace({ name: 'Login' })
        return true
      }
      // 留在设置页时**重读配置并复位 baseline**（评审十 #4）：切换改了
      // activeOj/ojUrl 却没动 baseline，dirty 会假真、Save 被误启用；且表单
      // 其余字段仍是旧 OJ 的值，保存会写到新 OJ（语义错误）。
      await load()
      return true
    } catch (e) {
      switchError.value = errorMessage(e, '切换 OJ 失败')
      return false
    }
  }

  async function onSwitchOj(): Promise<void> {
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
    if (await performSwitch(target)) {
      form.ojUrl = ojInstances.value.find((i) => i.id === target)?.baseUrl ?? ''
    } else {
      form.activeOj = persistedActive.value
    }
  }

  function populate(config: AppConfig): void {
    form.activeOj = config.oj.active
    persistedActive.value = config.oj.active
    switchError.value = null
    switchNotice.value = null
    ojInstances.value = config.oj.instances
    // active 无对应实例时保持空地址，绝不借用其它实例的 URL（否则保存会写错目标）
    form.ojUrl = config.oj.instances.find((i) => i.id === config.oj.active)?.baseUrl ?? ''
    form.contestRef = config.oj.contestRef ?? ''
    form.contestPassword = config.oj.contestPassword ?? ''
    form.timeoutSecs = String(config.oj.timeoutSecs)
    form.pollIntervalSecs = String(config.oj.pollIntervalSecs)
    form.pollTimeoutSecs = String(config.oj.pollTimeoutSecs)
    form.cacheTtlSecs = String(config.oj.cacheTtlSecs)
    form.cacheProblemStatement = config.oj.cacheProblemStatement ?? true
    form.fontSize = String(config.editor.fontSize)
    form.tabSize = EDITOR_TAB_SIZES.includes(config.editor.tabSize) ? config.editor.tabSize : 4
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
      configService.invalidate()
      populate(await configService.getConfig())
    } catch (e) {
      loadError.value = errorMessage(e, '读取配置失败')
    } finally {
      loading.value = false
    }
  }

  async function save(): Promise<void> {
    if (!canSave.value) return
    saving.value = true
    saveError.value = null
    // 固定提交快照：保存期间的后续编辑不进入本次落盘，也不得被误标为已保存
    const submitted = { ...form }
    const submittedSnapshot = snapshot()
    const target = submitted.activeOj
    const isNewInstance = !ojInstances.value.some((i) => i.id === target)
    try {
      // 锁定字段：不可编辑，写回未改动值即可——但**不参与数字解析**（存量非法
      // 字符串会变 NaN → JSON null → Rust u64 反序列化失败拖垮整次保存；
      // 评审十 #3）。保持磁盘原值不动。
      const lockAll = locked.value
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
      baseline.value = submittedSnapshot
      ojInstances.value = saved.oj.instances
      markSaved(true)

      if (isNewInstance) {
        if (await performSwitch(target)) return
        // 后端仍停留在旧 OJ：下拉和地址必须一起回滚，否则下一次无关保存会把
        // 新实例地址写进旧实例。baseline 同步到“已保存配置 + 回滚后的选择”，
        // 避免产生用户未做任何编辑却显示未保存的假 dirty；保存期间的后续编辑仍保留。
        const rollbackActive = persistedActive.value
        const rollbackUrl = ojInstances.value.find((i) => i.id === rollbackActive)?.baseUrl ?? ''
        form.activeOj = rollbackActive
        form.ojUrl = rollbackUrl
        baseline.value = JSON.stringify({ ...submitted, activeOj: rollbackActive, ojUrl: rollbackUrl })
        switchNotice.value = `${target} 实例已保存；切换未成功，可在上方重试`
      }
    } catch (e) {
      saveError.value = errorMessage(e, '保存配置失败')
    } finally {
      saving.value = false
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
    load,
    save,
    discard,
    onSwitchOj,
    inputClass,
  }
}
