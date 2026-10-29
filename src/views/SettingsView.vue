<script setup lang="ts">
import PageHeading from '@/components/common/PageHeading.vue'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import ErrorMessage from '@/components/common/ErrorMessage.vue'
import LoadingSpinner from '@/components/common/LoadingSpinner.vue'
import { ojBaseUrlHint } from '@/utils/oj'
import { useSettingsForm } from '@/composables/useSettingsForm'
import { useAboutSettings } from '@/composables/useAboutSettings'
import { useMaintenanceSettings } from '@/composables/useMaintenanceSettings'
import { useDataDirSettings } from '@/composables/useDataDirSettings'

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

const {
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
  clampRatio,
  inputClass,
  INPUT,
  LANGUAGE_OPTIONS,
  TAB_SIZES,
} = useSettingsForm()

const {
  storage,
  storageFailed,
  loadStorage,
  copiedKey,
  copyText,
} = useAboutSettings()

const {
  resetting,
  resetConfirmOpen,
  resetNotice,
  resetError,
  usage,
  usageFailed,
  purgeLogs,
  purgeSnapshots,
  purging,
  purgeConfirmOpen,
  purgeNotice,
  purgeError,
  resetClient,
  loadUsage,
  purgeLocalData,
  formatBytes,
} = useMaintenanceSettings()

const {
  dataDir,
  dataDirFailed,
  pendingDir,
  migrateData,
  changingDir,
  changeNotice,
  changeError,
  dataDirSourceMeta,
  loadDataDir,
  cancelDataDirChoice,
  chooseDataDir,
  confirmDataDir,
  restoreDefaultDataDir,
} = useDataDirSettings()

// ── 分组导航 scroll-spy ──
// 左侧索引导航与 sections 的顺序/数量一一对应；滚动时以「最后一个顶边越过
// 判定线的分组」为当前项，点击导航立即锁定高亮，平滑滚动结束后由滚动事件校正。

const SECTION_LABELS: readonly string[] = [
  'OJ 服务器',
  '编辑器',
  '布局',
  '主题',
  '重置与清理',
  '数据目录',
  '关于',
]

const activeSection = ref(0)
/// 页面纵向滚动容器（sections 渲染在其中）；scroll-spy 的判定基准
const scrollerRef = ref<HTMLElement | null>(null)

/// 判定线 = 滚动容器顶部下移 96px：分组标题进入这条线以上即视为「当前分组」
const SPY_OFFSET_PX = 96

/**
 * 点击导航后的 scroll-spy 锁定。
 *
 * 平滑滚动会途经中间各分组，若每个滚动帧都实时校正高亮，导航文字会在途中
 * 分组间来回跳动（闪烁）。点击时锁定到目标分组，滚动到位（目标顶边抵达
 * scroll-margin 停靠点 ±2px）后才把高亮交还给滚动驱动；超时兜底释放，
 * 防止滚动被打断（用户中途反拖滚轮）时锁死。
 */
let spyLockTarget: number | null = null
let spyLockTimer: ReturnType<typeof setTimeout> | null = null

function releaseSpyLock(): void {
  spyLockTarget = null
  if (spyLockTimer) {
    clearTimeout(spyLockTimer)
    spyLockTimer = null
  }
}

function updateActiveSection(): void {
  const scroller = scrollerRef.value
  if (!scroller) return
  const scrollerTop = scroller.getBoundingClientRect().top
  // 「滚到底」只在页面真的可滚动时成立 —— 视口足够高、无需滚动时
  // scrollHeight ≈ clientHeight，atBottom 恒真会把首屏误判成最后一个分组
  const scrollable = scroller.scrollHeight > scroller.clientHeight + 2
  const atBottom =
    scrollable && scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 2

  if (spyLockTarget !== null) {
    const target = document.getElementById(`settings-${spyLockTarget}`)
    // scroll-margin-top: 24px —— 平滑滚动的终点是「目标顶边 = 容器顶 + 24」。
    // atBottom 只对「目标是最后一个分组」算到位信号：从底部回点上方分组时，
    // 起滚瞬间仍处于底部，若把 atBottom 当作任意目标的到位信号，锁会提前
    // 释放、高亮随回滚扫过所有中间分组（即本要消除的闪烁）
    const docked =
      target !== null && Math.abs(target.getBoundingClientRect().top - scrollerTop - 24) <= 2
    const settled = docked || (atBottom && spyLockTarget === SECTION_LABELS.length - 1)
    if (!settled) return
    releaseSpyLock()
  }

  if (atBottom) {
    activeSection.value = SECTION_LABELS.length - 1
    return
  }

  const anchorTop = scrollerTop + SPY_OFFSET_PX
  let current = 0
  for (let i = 0; i < SECTION_LABELS.length; i++) {
    const el = document.getElementById(`settings-${i}`)
    if (el && el.getBoundingClientRect().top <= anchorTop) current = i
  }
  activeSection.value = current
}

function scrollToSection(index: number) {
  activeSection.value = index
  spyLockTarget = index
  if (spyLockTimer) clearTimeout(spyLockTimer)
  spyLockTimer = setTimeout(releaseSpyLock, 1_000)
  const target = document.getElementById(`settings-${index}`)
  target?.scrollIntoView({ block: 'start', behavior: 'smooth' })
  target?.focus({ preventScroll: true })
}

/// sections 仅在配置加载成功后渲染，渲染完成后再做一次高亮校正
watch(loading, (isLoading) => {
  if (!isLoading) void nextTick(updateActiveSection)
})

// ── 布局滑杆 ──

function onSplitInput(event: Event): void {
  const value = Number((event.target as HTMLInputElement).value)
  if (Number.isFinite(value)) form.splitRatio = clampRatio(value)
}

const splitPercent = computed(() => Math.round(form.splitRatio * 100))

onMounted(() => {
  void load()
  void loadStorage()
  void loadUsage()
  void loadDataDir()
})

onBeforeUnmount(() => {
  releaseSpyLock()
})
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col overflow-hidden bg-[var(--bg-body)]">
    <div ref="scrollerRef" class="min-h-0 flex-1 overflow-y-auto" @scroll.passive="updateActiveSection">
      <div class="settings-page mx-auto w-full max-w-[1250px] px-8 pb-8">
        <PageHeading title="环境设置" index="05" subtitle="PREFERENCES">
          <template #description>配置保存在本地客户端，仅影响当前设备</template>
        </PageHeading>
        <nav v-if="!loading && !loadError" class="settings-index" aria-label="设置分组">
          <a
            v-for="(label, index) in SECTION_LABELS"
            :key="label"
            :href="`#settings-${index}`"
            :class="{ active: activeSection === index }"
            :aria-current="activeSection === index ? 'true' : undefined"
            @click.prevent="scrollToSection(index)"
          ><span>{{ String(index + 1).padStart(2, '0') }}</span>{{ label }}</a>
        </nav>
        <LoadingSpinner v-if="loading" message="正在加载配置…" />
        <ErrorMessage v-else-if="loadError" :message="loadError" :retry="load" />

        <template v-else>
          <div class="settings-sections flex flex-col gap-8">
            <!-- ── OJ 服务器 ── -->
            <section id="settings-0" tabindex="-1" class="settings-section">
              <div class="flex items-center gap-2 border-b border-[var(--border-color)] px-5 py-3.5">
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
                  <span v-if="switchError" class="mt-1 block text-xs text-[var(--color-error)]">
                    {{ switchError }}
                  </span>
                  <!-- 引导/安抚提示与错误**并存**：两者语义独立（如「实例已保存但切换未成功」
                       既是错误也需要安抚），用 v-else-if 会让提示永不可见 -->
                  <span v-if="switchNotice" class="mt-1 block text-xs text-[var(--color-warning)]">
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
                  <span v-if="errors.ojUrl" class="mt-1 block text-xs text-[var(--color-error)]">
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
                  <span v-if="errors.timeoutSecs" class="mt-1 block text-xs text-[var(--color-error)]">
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
                  <span v-if="errors.pollIntervalSecs" class="mt-1 block text-xs text-[var(--color-error)]">
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
                  <span v-if="errors.pollTimeoutSecs" class="mt-1 block text-xs text-[var(--color-error)]">
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
                  <span v-if="errors.cacheTtlSecs" class="mt-1 block text-xs text-[var(--color-error)]">
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
                      class="settings-switch relative h-6 w-11 shrink-0 transition-colors"
                      :class="
                        form.cacheProblemStatement ? 'bg-[var(--color-primary)]' : 'bg-[var(--bg-sidebar)]'
                      "
                      @click="form.cacheProblemStatement = !form.cacheProblemStatement"
                    >
                      <span
                        class="settings-switch-knob absolute top-0.5 left-0.5 h-5 w-5 rounded-sm bg-[var(--bg-card)] transition-transform"
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
            <section id="settings-1" tabindex="-1" class="settings-section">
              <div class="flex items-center gap-2 border-b border-[var(--border-color)] px-5 py-3.5">
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
                  <span v-if="errors.fontSize" class="mt-1 block text-xs text-[var(--color-error)]">
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
                      class="settings-switch relative h-6 w-11 shrink-0 transition-colors"
                      :class="form.autoSave ? 'bg-[var(--color-primary)]' : 'bg-[var(--bg-sidebar)]'"
                      @click="form.autoSave = !form.autoSave"
                    >
                      <span
                        class="settings-switch-knob absolute left-0.5 top-0.5 h-5 w-5 rounded-sm bg-[var(--bg-card)] transition-transform"
                        :class="form.autoSave ? 'translate-x-5' : ''"
                      ></span>
                    </button>
                    <span class="text-xs text-[var(--text-secondary)]">
                      {{ form.autoSave ? '已开启' : '已关闭' }}
                    </span>
                  </div>
                  <span class="mt-1 block text-xs text-[var(--text-muted)]">
                    定时保存解题页代码，崩溃后可恢复；关闭后仅在切题、失焦、离开解题页或关窗时落盘
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
                    class="mt-1 block text-xs text-[var(--color-error)]"
                  >
                    {{ errors.autoSaveIntervalSecs }}
                  </span>
                  <span v-else class="mt-1 block text-xs text-[var(--text-muted)]">5–300</span>
                </label>
              </div>
            </section>

            <!-- ── 布局 ── -->
            <section id="settings-2" tabindex="-1" class="settings-section">
              <div class="flex items-center gap-2 border-b border-[var(--border-color)] px-5 py-3.5">
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
                  class="mt-1 flex justify-between font-mono text-xs text-[var(--text-muted)]"
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
            <section id="settings-3" tabindex="-1" class="settings-section">
              <div class="flex items-center gap-2 border-b border-[var(--border-color)] px-5 py-3.5">
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
            <section id="settings-4" tabindex="-1" class="settings-section">
              <div class="flex items-center gap-2 border-b border-[var(--border-color)] px-5 py-3.5">
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

              <div class="divide-y divide-[var(--border-color)]">
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
                      <span v-if="resetError" class="text-xs text-[var(--color-error)]">{{ resetError }}</span>
                      <span
                        v-else-if="resetNotice"
                        class="text-xs"
                        :class="resetNotice.warn ? 'text-[var(--color-warning)]' : 'text-[var(--color-success)]'"
                      >
                        {{ resetNotice.text }}
                      </span>

                      <template v-if="resetConfirmOpen || resetting">
                        <button
                          v-if="!resetting"
                          type="button"
                          class="rounded-sm border border-[var(--border-color)] px-3 py-1.5 text-xs font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-sidebar)]"
                          @click="resetConfirmOpen = false"
                        >
                          取消
                        </button>
                        <button
                          type="button"
                          class="rounded-sm bg-[var(--color-primary)] px-3 py-1.5 text-xs font-semibold text-white transition-colors hover:brightness-110 disabled:opacity-60"
                          :disabled="resetting"
                          @click="resetClient"
                        >
                          {{ resetting ? '重置中…' : '确认重置' }}
                        </button>
                      </template>
                      <button
                        v-else
                        type="button"
                        class="rounded-sm border border-[var(--border-color)] px-3 py-1.5 text-xs font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-sidebar)]"
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
                    —— 删除<strong class="font-semibold text-[var(--color-error)]">不可重建</strong>的本地内容。
                    日志是排障线索，提交源码留档是 OJ 不回吐代码时的唯一来源，删掉都无法恢复。
                  </p>

                  <!-- 体积预览：不可逆动作必须让用户看到确切范围 -->
                  <div
                    class="mt-3 flex flex-col gap-2 rounded-sm border border-[var(--border-color)] bg-[var(--bg-sidebar)]/70 px-3.5 py-3"
                  >
                    <p v-if="usageFailed" class="text-xs text-[var(--color-error)]">占用信息获取失败</p>
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
                    <p class="min-w-0 flex-1 text-xs leading-relaxed text-[var(--text-muted)]">
                      此操作<strong class="font-semibold text-[var(--color-error)]">不可撤销</strong>；
                      不会删除工作区代码、配置、登录会话与公告已读状态。
                    </p>
                    <div class="flex shrink-0 items-center gap-2">
                      <span v-if="purgeError" class="text-xs text-[var(--color-error)]">{{ purgeError }}</span>
                      <span
                        v-else-if="purgeNotice"
                        class="text-xs"
                        :class="purgeNotice.warn ? 'text-[var(--color-warning)]' : 'text-[var(--color-success)]'"
                      >
                        {{ purgeNotice.text }}
                      </span>

                      <template v-if="purgeConfirmOpen || purging">
                        <button
                          v-if="!purging"
                          type="button"
                          class="rounded-sm border border-[var(--border-color)] px-3 py-1.5 text-xs font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-sidebar)]"
                          @click="purgeConfirmOpen = false"
                        >
                          取消
                        </button>
                        <button
                          type="button"
                          class="rounded-sm bg-[var(--color-error)] px-3 py-1.5 text-xs font-semibold text-white transition-colors hover:bg-[var(--color-error)] disabled:opacity-60"
                          :disabled="purging"
                          @click="purgeLocalData"
                        >
                          {{ purging ? '清理中…' : '确认清理（不可恢复）' }}
                        </button>
                      </template>
                      <button
                        v-else
                        type="button"
                        class="rounded-sm border border-[var(--border-color)] px-3 py-1.5 text-xs font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-sidebar)]"
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
            <section id="settings-5" tabindex="-1" class="settings-section">
              <div class="flex items-center gap-2 border-b border-[var(--border-color)] px-5 py-3.5">
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
                <p v-if="dataDirFailed" class="text-xs text-[var(--color-error)]">数据目录信息获取失败</p>
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
                    class="rounded-sm border border-[var(--color-wa-border)] bg-[var(--color-wa-bg)] px-3 py-2 text-xs leading-relaxed text-[var(--color-error)]"
                  >
                    默认数据目录不可用，当前数据存在<strong class="font-semibold">临时目录</strong>中
                    —— 系统清理临时文件时会连带删除你的代码与提交留档，请尽快更改数据目录。
                  </p>

                  <!-- 待重启提示：改动已记录但尚未生效，必须说清「现在仍在用旧目录」 -->
                  <p
                    v-if="dataDir.restartRequired"
                    class="rounded-sm border border-[var(--color-pending-border)] bg-[var(--color-pending-bg)] px-3 py-2 text-xs leading-relaxed text-[var(--color-warning)]"
                  >
                    已记录目录更改，<strong class="font-semibold">重启客户端后生效</strong>；
                    当前仍在使用上面的目录。
                  </p>

                  <!-- 选择器已返回候选路径：确认 + 是否迁移 -->
                  <div
                    v-if="pendingDir"
                    class="flex flex-col gap-2 rounded-sm border border-[var(--border-color)] bg-[var(--bg-sidebar)]/70 px-3.5 py-3"
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
                    <p class="min-w-0 flex-1 text-xs leading-relaxed text-[var(--text-muted)]">
                      数据目录决定工作区代码、提交留档、配置与日志的存放位置。
                      更改后需重启客户端生效。
                    </p>
                    <div class="flex shrink-0 items-center gap-2">
                      <span v-if="changeError" class="text-xs text-[var(--color-error)]">{{ changeError }}</span>
                      <span v-else-if="changeNotice" class="text-xs text-[var(--color-warning)]">
                        {{ changeNotice }}
                      </span>

                      <template v-if="pendingDir">
                        <button
                          type="button"
                          class="rounded-sm border border-[var(--border-color)] px-3 py-1.5 text-xs font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-sidebar)]"
                          :disabled="changingDir"
                          @click="cancelDataDirChoice"
                        >
                          取消
                        </button>
                        <button
                          type="button"
                          class="rounded-sm bg-[var(--color-primary)] px-3 py-1.5 text-xs font-semibold text-white transition-colors hover:brightness-110 disabled:opacity-60"
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
                          class="rounded-sm border border-[var(--border-color)] px-3 py-1.5 text-xs font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-sidebar)] disabled:opacity-60"
                          :disabled="changingDir"
                          @click="restoreDefaultDataDir"
                        >
                          恢复默认
                        </button>
                        <button
                          type="button"
                          class="rounded-sm border border-[var(--border-color)] px-3 py-1.5 text-xs font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-sidebar)] disabled:opacity-60"
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
            <section id="settings-6" tabindex="-1" class="settings-section">
              <div class="flex items-center gap-2 border-b border-[var(--border-color)] px-5 py-3.5">
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
              <dl class="divide-y divide-[var(--border-color)] px-5">
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
                      class="shrink-0 rounded-md border border-[var(--border-color)] px-2 py-0.5 text-xs text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-sidebar)]"
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
                      class="shrink-0 rounded-md border border-[var(--border-color)] px-2 py-0.5 text-xs text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-sidebar)]"
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
                      class="shrink-0 rounded-md border border-[var(--border-color)] px-2 py-0.5 text-xs text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-sidebar)]"
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
                    :class="storageFailed ? 'text-[var(--color-error)]' : 'text-[var(--text-muted)]'"
                  >
                    {{ storageFailed ? '获取失败' : '正在获取…' }}
                  </dd>
                </div>
              </dl>
            </section>
          </div>

          <!-- 底部粘性操作栏 -->
          <div
            class="sticky bottom-0 z-10 mt-4 rounded-sm border border-[var(--border-color)] bg-[var(--bg-card)]/95 px-4 py-3 shadow-lg backdrop-blur"
          >
            <div
              v-if="saveError"
              class="mb-2 rounded-sm border border-[var(--color-wa-border)] bg-[var(--color-wa-bg)] px-3 py-2 text-xs text-[var(--color-error)]"
            >
              保存失败：{{ saveError }}
            </div>
            <div class="flex items-center justify-between gap-3">
              <div class="min-w-0 text-xs">
                <span
                  v-if="showSaved"
                  class="inline-flex items-center gap-1.5 rounded-sm border border-[var(--color-ac-border)] bg-[var(--color-ac-bg)] px-2.5 py-1 font-medium text-[var(--color-success)]"
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
                <span v-else-if="dirty && !isValid" class="text-[var(--color-error)]">
                  存在校验错误，请修正后再保存
                </span>
                <span v-else-if="dirty" class="text-[var(--color-warning)]">有未保存的更改</span>
                <span v-else class="text-[var(--text-muted)]">所有更改已保存</span>
              </div>
              <div class="flex shrink-0 items-center gap-2">
                <button
                  type="button"
                  class="rounded-sm border border-[var(--border-color)] bg-[var(--bg-card)] px-4 py-1.5 text-sm text-[var(--text-secondary)] transition-colors enabled:hover:bg-[var(--bg-sidebar)] disabled:cursor-not-allowed disabled:opacity-40"
                  :disabled="!dirty || saving"
                  @click="discard"
                >
                  放弃更改
                </button>
                <button
                  type="button"
                  class="rounded-sm bg-[var(--color-primary)] px-4 py-1.5 text-sm font-medium text-white transition-colors enabled:hover:bg-[var(--color-primary-hover)] disabled:cursor-not-allowed disabled:opacity-40"
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

<style scoped>
.settings-page { position: relative; }
.settings-page :deep(.page-heading) { padding-left: 0; padding-right: 0; }
.settings-switch { border-radius: 2px; }
.settings-switch-knob { box-shadow: 0 1px 3px #2e303326; }
.settings-index { position: sticky; top: 24px; float: left; display: flex; flex-direction: column; gap: 6px; width: 175px; padding-top: 16px; }
.settings-index a { position: relative; display: flex; gap: 16px; padding: 11px 0 11px 14px; color: var(--text-secondary); font-size: 12px; transition: color 150ms; }
.settings-index a::before { content: ''; position: absolute; left: 0; top: 10px; bottom: 10px; width: 2px; border-radius: 1px; background: transparent; transition: background 150ms; }
.settings-index a:hover { color: #8c6a43; }
/* 当前分组：棕色高亮 + 左侧指示条（与全站 hover 棕 #8c6a43 一致） */
.settings-index a.active { color: #8c6a43; font-weight: 600; }
.settings-index a.active::before { background: #8c6a43; }
.settings-index span { font: 12px var(--font-mono); color: var(--text-muted); }
.settings-index a.active span { color: #8c6a43; }
.settings-sections { margin-left: 205px; }
.settings-section { scroll-margin-top: 24px; border-top: 1px solid var(--border-color); }
.settings-section > div:first-child { border: 0; padding: 20px 0 8px; }
.settings-section > div:first-child svg { display: none; }
.settings-section h2 { font-size: 18px; font-weight: 600; }
.settings-section > div:not(:first-child) { padding-left: 0; padding-right: 0; }
.settings-section :is(input:not([type=range]):not([type=checkbox]), select) { border-radius: 0; border-width: 0 0 1px; background: transparent; padding-left: 0; }
.settings-section input[inputmode="numeric"] { font-family: var(--font-mono); font-variant-numeric: tabular-nums; }
@media (max-width: 1100px) { .settings-index { width: 140px; } .settings-sections { margin-left: 165px; } }
</style>
