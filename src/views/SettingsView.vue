<script setup lang="ts">
import PageHeading from '@/components/common/PageHeading.vue'
import { onMounted, ref } from 'vue'
import ErrorMessage from '@/components/common/ErrorMessage.vue'
import LoadingSpinner from '@/components/common/LoadingSpinner.vue'
import { useSettingsForm } from '@/composables/useSettingsForm'
import { useAboutSettings } from '@/composables/useAboutSettings'
import { useMaintenanceSettings } from '@/composables/useMaintenanceSettings'
import { useDataDirSettings } from '@/composables/useDataDirSettings'
import { useSettingsScrollSpy } from '@/composables/useSettingsScrollSpy'
import { systemService } from '@/services/system.service'
import type { SettingsFormPatch } from '@/composables/useSettingsForm'
import OjServerSection from '@/components/settings/OjServerSection.vue'
import EditorSection from '@/components/settings/EditorSection.vue'
import LayoutSection from '@/components/settings/LayoutSection.vue'
import ThemeSection from '@/components/settings/ThemeSection.vue'
import ResetPurgeSection from '@/components/settings/ResetPurgeSection.vue'
import DataDirSection from '@/components/settings/DataDirSection.vue'
import AboutSection from '@/components/settings/AboutSection.vue'

/**
 * 设置页 —— 应用配置（AppConfig）的可视化编辑入口。
 *
 * 数据流：onMounted 先 `invalidate()` 再 `getConfig()`，保证页面展示的恒为磁盘真值
 * （其它页面可能已热改过配置，进程内缓存不可信）；编辑在本地草稿上进行，
 * 保存时经 `configService.updateConfig`（读-改-写整体替换）落盘并失效缓存。
 *
 * 分层锁：连接与缓存调优字段（OJ/地址/超时/轮询/TTL/数据目录）默认锁定，
 * 唯一提权来源是 `--settings` 启动参数（管理方钥匙）；选手日常字段（比赛
 * ID/引用、密码、编辑器、布局、重置与清理、关于）不受影响。护栏而非安全
 * 边界 —— config.json 明文可手改，无后端强制。
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
  inputClass,
} = useSettingsForm()

/// 维护者层锁定（默认锁，`--settings` 解锁）。守卫在首次导航时已预热
/// getLaunchFlags 的 memoized Promise，此处 await 即时完成，无可见闪烁。
const locked = ref(true)

function updateForm(patch: SettingsFormPatch): void {
  Object.assign(form, patch)
}

const about = useAboutSettings()

const maintenance = useMaintenanceSettings()

const dataDirSettings = useDataDirSettings()

const {
  activeSection,
  scrollerRef,
  scheduleActiveSection,
  scrollToSection,
  SECTION_LABELS,
} = useSettingsScrollSpy(loading)

onMounted(() => {
  void systemService.getLaunchFlags().then((flags) => {
    locked.value = !flags.settings
  })
  void load()
  void about.loadStorage()
  void maintenance.loadUsage()
  void dataDirSettings.loadDataDir()
})
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col overflow-hidden bg-[var(--bg-body)]">
    <div ref="scrollerRef" class="min-h-0 flex-1 overflow-y-auto" @scroll.passive="scheduleActiveSection">
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
            <OjServerSection
              :form="form"
              :errors="errors"
              :options="ojOptions"
              :switch-error="switchError"
              :switch-notice="switchNotice"
              :input-class="inputClass"
              :locked="locked"
              @update:form="updateForm"
              @switch-oj="onSwitchOj"
            />

            <!-- ── 编辑器 ── -->
            <EditorSection
              :form="form"
              :errors="errors"
              :input-class="inputClass"
              @update:form="updateForm"
            />

            <!-- ── 布局 ── -->
            <LayoutSection :split-ratio="form.splitRatio" @update:form="updateForm" />

            <!-- ── 主题 ── -->
            <ThemeSection />

            <!-- ── 重置与清理 ── -->
            <ResetPurgeSection :model="maintenance" />

            <!-- ── 数据目录 ── -->
            <DataDirSection :model="dataDirSettings" :locked="locked" />

            <!-- ── 关于 ── -->
            <AboutSection :model="about" />
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
                  v-if="showSaved && !dirty"
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
@media (max-width: 1100px) { .settings-index { width: 140px; } .settings-sections { margin-left: 165px; } }
</style>
