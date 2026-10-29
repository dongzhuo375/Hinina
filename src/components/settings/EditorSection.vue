<script setup lang="ts">
import SettingsSectionFrame from './SettingsSectionFrame.vue'
import { DEFAULT_LANGUAGES } from '@/utils/language'
import { EDITOR_TAB_SIZES } from '@/utils/editor'
import type { SettingsForm, SettingsFormPatch } from '@/composables/useSettingsForm'

const props = defineProps<{
  form: SettingsForm
  errors: Record<string, string | null>
  inputClass: (field: string) => string
  input: string
}>()
const emit = defineEmits<{ (event: 'update:form', patch: SettingsFormPatch): void }>()
const { form, inputClass, input: INPUT } = props
const LANGUAGE_OPTIONS = DEFAULT_LANGUAGES
const TAB_SIZES = EDITOR_TAB_SIZES

</script>

<template>
  <SettingsSectionFrame id="settings-1">
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
                  <span v-if="props.errors.fontSize" class="mt-1 block text-xs text-[var(--color-error)]">
                    {{ props.errors.fontSize }}
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
                      @click="emit('update:form', { autoSave: !form.autoSave })"
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
                    v-if="props.errors.autoSaveIntervalSecs"
                    class="mt-1 block text-xs text-[var(--color-error)]"
                  >
                    {{ props.errors.autoSaveIntervalSecs }}
                  </span>
                  <span v-else class="mt-1 block text-xs text-[var(--text-muted)]">5–300</span>
                </label>
              </div>
  </SettingsSectionFrame>
</template>
